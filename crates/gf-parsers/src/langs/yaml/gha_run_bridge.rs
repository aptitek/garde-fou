//! Cross-AST bridge from GitHub Actions `run:` steps to `tree-sitter-bash` (`Bash` / `PosixSh`)
//! and `tree-sitter-powershell` (`Pwsh`), preserving exact 1-based YAML line spans.

use gf_core::{LangRuleFinding, ShellDialect, Span};

use super::anchors_merge::ResolvedYaml;
use super::events_tree::YamlScalar;
use crate::langs::powershell::analyze_dockerfile_powershell_run;
use crate::langs::shell::analyze_embedded_shell_snippet;

/// Inherited workflow-level and job-level shell defaults for GitHub Actions `run:` steps.
#[derive(Clone, Copy, Debug, Default)]
pub struct JobShellDefaults<'a> {
    /// Optional `defaults.run.shell` at workflow root.
    pub workflow_shell: Option<&'a str>,
    /// Optional `jobs.<id>.defaults.run.shell`.
    pub job_shell: Option<&'a str>,
    /// Optional `jobs.<id>.runs-on` scalar string.
    pub runs_on: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GhaShellKind {
    Bash { default_errexit: bool },
    PosixSh { default_errexit: bool },
    Pwsh,
    Other,
}

fn parse_explicit_shell(raw: &str) -> GhaShellKind {
    let mut parts = raw.split_whitespace();
    let Some(first) = parts.next() else {
        return GhaShellKind::Other;
    };
    let bin = first.rsplit(['/', '\\']).next().unwrap_or(first);
    let rest: Vec<&str> = parts.collect();
    let is_builtin_shorthand = rest.is_empty();
    let has_e_flag = is_builtin_shorthand
        || rest
            .iter()
            .any(|a| !a.starts_with("--") && a.strip_prefix('-').is_some_and(|f| f.contains('e')));
    match bin.to_ascii_lowercase().as_str() {
        "bash" => GhaShellKind::Bash {
            default_errexit: has_e_flag,
        },
        "sh" | "dash" | "ash" => GhaShellKind::PosixSh {
            default_errexit: has_e_flag,
        },
        "pwsh" | "powershell" => GhaShellKind::Pwsh,
        _ => GhaShellKind::Other,
    }
}

fn step_targets_windows(step: &ResolvedYaml, runs_on: Option<&str>) -> bool {
    if runs_on.is_some_and(|ro| ro.to_ascii_lowercase().contains("windows")) {
        return true;
    }
    step.get("if")
        .and_then(ResolvedYaml::as_scalar)
        .is_some_and(|cond| {
            let lower = cond.value.to_ascii_lowercase();
            lower.contains("runner.os == 'windows'") || lower.contains("runner.os == \"windows\"")
        })
}

fn resolve_step_shell(step: &ResolvedYaml, defaults: &JobShellDefaults<'_>) -> GhaShellKind {
    let explicit = step
        .get("shell")
        .and_then(ResolvedYaml::as_scalar)
        .map(|s| s.value.as_str())
        .or(defaults.job_shell)
        .or(defaults.workflow_shell);
    if let Some(raw) = explicit {
        return parse_explicit_shell(raw);
    }
    if step_targets_windows(step, defaults.runs_on) {
        GhaShellKind::Pwsh
    } else {
        GhaShellKind::Bash {
            default_errexit: true,
        }
    }
}

/// Replaces `${{ ... }}` with `__GHA_EXPR__`, preserving exact `\n` count via line continuations.
fn scrub_gha_expressions_preserving_lines(script: &str, continuation: char) -> String {
    let mut segs = script.split("${{");
    let Some(first) = segs.next() else {
        return String::new();
    };
    let mut out = String::from(first);
    for seg in segs {
        if let Some((inner, after_close)) = seg.split_once("}}") {
            out.push_str("__GHA_EXPR__");
            for _ in 0..inner.bytes().filter(|&b| b == b'\n').count() {
                out.push(' ');
                out.push(continuation);
                out.push('\n');
            }
            out.push_str(after_close);
        } else {
            out.push_str("${{");
            out.push_str(seg);
        }
    }
    out
}

fn remap_gha_span(run_scalar: &YamlScalar, inner: Span) -> Span {
    let start_line = run_scalar.remap_embedded_line(inner.start_line());
    let end_line = run_scalar
        .remap_embedded_line(inner.end_line())
        .max(start_line);
    Span::new(start_line, end_line).unwrap_or(run_scalar.span)
}

fn append_remapped(
    run_scalar: &YamlScalar,
    findings: Vec<LangRuleFinding>,
    out: &mut Vec<LangRuleFinding>,
) {
    for mut f in findings {
        f.span = remap_gha_span(run_scalar, f.span);
        out.push(f);
    }
}

fn run_shell_bridge(
    dialect: ShellDialect,
    default_errexit: bool,
    run_scalar: &YamlScalar,
    out: &mut Vec<LangRuleFinding>,
) {
    let scrubbed = scrub_gha_expressions_preserving_lines(&run_scalar.value, '\\');
    if let Ok(bridged) = analyze_embedded_shell_snippet(dialect, &scrubbed, default_errexit) {
        append_remapped(run_scalar, bridged.findings, out);
    }
}

/// Bridges a GitHub Actions `run:` scalar into the corresponding Shell or PowerShell CST analyzer.
pub fn bridge_gha_step_run(
    step: &ResolvedYaml,
    run_scalar: &YamlScalar,
    defaults: &JobShellDefaults<'_>,
    out: &mut Vec<LangRuleFinding>,
) {
    match resolve_step_shell(step, defaults) {
        GhaShellKind::Bash { default_errexit } => {
            run_shell_bridge(ShellDialect::Bash, default_errexit, run_scalar, out);
        }
        GhaShellKind::PosixSh { default_errexit } => {
            run_shell_bridge(ShellDialect::PosixSh, default_errexit, run_scalar, out);
        }
        GhaShellKind::Pwsh => {
            let scrubbed = scrub_gha_expressions_preserving_lines(&run_scalar.value, '`');
            if let Ok(findings) = analyze_dockerfile_powershell_run(&scrubbed) {
                append_remapped(run_scalar, findings, out);
            }
        }
        GhaShellKind::Other => {}
    }
}
