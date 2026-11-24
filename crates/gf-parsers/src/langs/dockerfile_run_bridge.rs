//! Cross-language AST bridge delegating `Dockerfile RUN` layers to `tree-sitter-bash`
//! (`PosixSh` / `Bash`) or `tree-sitter-powershell` with exact 1-based line remapping.

use gf_core::{DockerCommandForm, DockerShellKind, LangRuleFinding, ShellDialect, Span};

use super::dockerfile_args::parse_docker_command_form;
use super::powershell::analyze_dockerfile_powershell_run;
use super::shell::{BridgedShellRun, analyze_dockerfile_shell_run, dialect::detect_shell_dialect};
use crate::types::ParseError;

/// Inputs needed to extract a `Dockerfile RUN` script with exact line offsets.
pub(super) struct RunBuildEnv<'a> {
    /// Raw lines of the `Dockerfile`.
    pub source_lines: &'a [&'a str],
    /// Active line-continuation escape character (`\` or `` ` ``).
    pub escape: char,
    /// Active stage `SHELL` dialect.
    pub shell_kind: DockerShellKind,
}

/// Extracted `RUN` payload with per-line mapping back to 1-based `Dockerfile` lines.
pub(super) struct ExtractedRunScript {
    /// Syntactic invocation form (`ExecForm` vs `ShellForm`).
    pub form: DockerCommandForm,
    /// Collapsed single-line command text for package-manager / `sudo` checks.
    pub collapsed_command: String,
    /// Multi-line script body preserving line breaks for sub-AST parsing.
    pub script: String,
    /// 1-based `Dockerfile` line number for each line in `script`.
    pub line_map: Vec<u32>,
    /// Whether this `RUN` used a `<<EOF` heredoc (allowing inner `#!/bin/bash` override).
    pub is_heredoc: bool,
}

fn strip_run_keyword_and_flags(first_line: &str) -> &str {
    let trimmed = first_line.trim_start();
    let kw_len = trimmed.split_whitespace().next().map_or(0, str::len);
    let mut rem = trimmed[kw_len..].trim_start();
    while let Some(tok) = rem.split_whitespace().next() {
        if tok.starts_with("--") {
            rem = rem[tok.len()..].trim_start();
        } else {
            return rem;
        }
    }
    ""
}

fn convert_continuation_for_shell(
    raw_line: &str,
    escape: char,
    shell_kind: DockerShellKind,
) -> String {
    let trimmed_end = raw_line.trim_end();
    let Some(without_esc) = trimmed_end.strip_suffix(escape) else {
        return String::from(trimmed_end);
    };
    if without_esc.ends_with(escape) {
        return String::from(trimmed_end);
    }
    let target_cont = match shell_kind {
        DockerShellKind::PowerShell => '`',
        _ => '\\',
    };
    format!("{}{target_cont}", without_esc.trim_end())
}

fn build_heredoc_run_script(slice: &[&str], start_line: u32) -> ExtractedRunScript {
    let body_slice = &slice[1..slice.len().saturating_sub(1)];
    let mut script = String::new();
    let mut line_map = Vec::with_capacity(body_slice.len());
    for (i, &line) in body_slice.iter().enumerate() {
        if i > 0 {
            script.push('\n');
        }
        script.push_str(line.trim_end_matches('\r'));
        line_map.push(
            start_line
                .saturating_add(1)
                .saturating_add(u32::try_from(i).unwrap_or(0)),
        );
    }
    let collapsed = body_slice
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    ExtractedRunScript {
        form: DockerCommandForm::ShellForm,
        collapsed_command: collapsed,
        script,
        line_map,
        is_heredoc: true,
    }
}

fn build_continued_run_script(
    slice: &[&str],
    start_line: u32,
    collapsed_args: &str,
    env: &RunBuildEnv<'_>,
) -> ExtractedRunScript {
    let mut script = String::new();
    let mut line_map = Vec::with_capacity(slice.len());
    for (offset, &raw_line) in slice.iter().enumerate() {
        let clean_cr = raw_line.trim_end_matches('\r');
        if clean_cr.trim_start().starts_with('#') {
            continue;
        }
        let line_content = if offset == 0 {
            strip_run_keyword_and_flags(clean_cr)
        } else {
            clean_cr
        };
        if !line_map.is_empty() {
            script.push('\n');
        }
        script.push_str(&convert_continuation_for_shell(
            line_content,
            env.escape,
            env.shell_kind,
        ));
        line_map.push(start_line.saturating_add(u32::try_from(offset).unwrap_or(0)));
    }
    ExtractedRunScript {
        form: DockerCommandForm::ShellForm,
        collapsed_command: String::from(collapsed_args),
        script,
        line_map,
        is_heredoc: false,
    }
}

/// Builds an [`ExtractedRunScript`] from the raw `Dockerfile` lines of a `RUN` block.
#[must_use]
pub(super) fn build_run_script(
    env: &RunBuildEnv<'_>,
    span: Span,
    collapsed_args: &str,
) -> ExtractedRunScript {
    let form = parse_docker_command_form(collapsed_args);
    let start_idx = usize::try_from(span.start_line().saturating_sub(1)).unwrap_or(0);
    let end_idx = usize::try_from(span.end_line()).unwrap_or(env.source_lines.len());
    let slice = env
        .source_lines
        .get(start_idx..end_idx.min(env.source_lines.len()))
        .unwrap_or(&[]);

    if matches!(form, DockerCommandForm::ExecForm(_)) || slice.is_empty() {
        return ExtractedRunScript {
            form,
            collapsed_command: String::from(collapsed_args),
            script: String::new(),
            line_map: Vec::new(),
            is_heredoc: false,
        };
    }
    if strip_run_keyword_and_flags(slice[0]).contains("<<") && slice.len() >= 2 {
        return build_heredoc_run_script(slice, span.start_line());
    }
    build_continued_run_script(slice, span.start_line(), collapsed_args, env)
}

fn remap_span(sub_span: Span, line_map: &[u32], fallback: Span) -> Span {
    let map_line = |sub_line: u32| -> u32 {
        let idx = usize::try_from(sub_line.saturating_sub(1)).unwrap_or(0);
        line_map
            .get(idx)
            .copied()
            .or_else(|| line_map.last().copied())
            .unwrap_or_else(|| fallback.start_line())
    };
    let s = map_line(sub_span.start_line());
    let e = map_line(sub_span.end_line()).max(s);
    Span::new(s, e).unwrap_or(fallback)
}

fn resolve_run_shell_dialect(
    run: &ExtractedRunScript,
    stage_shell: DockerShellKind,
) -> ShellDialect {
    if run.is_heredoc
        && run
            .script
            .lines()
            .next()
            .is_some_and(|l| l.trim_start().starts_with("#!"))
        && let Ok(dummy) = gf_core::RepoPath::new("Dockerfile.run.sh")
    {
        detect_shell_dialect(&dummy, &run.script)
    } else if stage_shell == DockerShellKind::Bash {
        ShellDialect::Bash
    } else {
        ShellDialect::PosixSh
    }
}

/// Evaluates an [`ExtractedRunScript`] with the stage's active `SHELL` CST analyzer and remaps spans.
///
/// # Errors
///
/// Returns [`ParseError`] if `Tree-sitter` grammar initialization fails.
pub(super) fn analyze_and_remap_run_script(
    run: &ExtractedRunScript,
    stage_shell: DockerShellKind,
    instr_span: Span,
    out_findings: &mut Vec<LangRuleFinding>,
) -> Result<bool, ParseError> {
    if run.script.trim().is_empty() {
        return Ok(false);
    }
    match stage_shell {
        DockerShellKind::PosixSh | DockerShellKind::Bash => {
            let dialect = resolve_run_shell_dialect(run, stage_shell);
            let res: BridgedShellRun = analyze_dockerfile_shell_run(dialect, &run.script)?;
            for mut f in res.findings {
                f.span = remap_span(f.span, &run.line_map, instr_span);
                out_findings.push(f);
            }
            Ok(res.has_unguarded_pipe)
        }
        DockerShellKind::PowerShell => {
            for mut f in analyze_dockerfile_powershell_run(&run.script)? {
                f.span = remap_span(f.span, &run.line_map, instr_span);
                out_findings.push(f);
            }
            Ok(false)
        }
        DockerShellKind::Other => Ok(false),
    }
}
