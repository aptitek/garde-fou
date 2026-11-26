//! Pure `tree-sitter-bash` CST extractor for POSIX `sh` & `bash` scripts and `Dockerfile RUN` layers.

pub mod bashisms;
mod curl_pipe;
pub mod dialect;
mod hazards;
mod metrics;
mod quoting;
mod strict_mode;

use std::borrow::Cow;

use gf_core::{LangRuleFinding, Language, RepoPath, ShellDialect, UastDocument};

use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{collect_syntax_errors, line_metrics, parse_with_language};
use crate::types::{LanguageExtractor, ParseError};
use bashisms::{check_posix_bashisms, has_unguarded_shell_pipeline};
use dialect::{detect_shell_dialect, is_sourced_shell_library};
use hazards::check_shell_hazards;
use metrics::collect_shell_uast_metrics;
use quoting::check_unquoted_expansions;
use strict_mode::{check_strict_mode_header, scan_top_level_strict_flags};

/// Neutralizes `tree-sitter-bash` 0.25 C-scanner UB where `isdigit(lexer->lookahead)`
/// (`scanner.c:1158,1172` inside `brace_start`) indexes glibc's 8-bit `ctype` table with `> 0xFF`.
const fn is_scanner_ub_char(c: char) -> bool {
    (c as u32) > 0xFF
}

fn sanitize_bash_scanner_ub(source: &str) -> Cow<'_, str> {
    if !source.contains('{') || !source.chars().any(is_scanner_ub_char) {
        return Cow::Borrowed(source);
    }
    Cow::Owned(
        source
            .chars()
            .map(|c| if is_scanner_ub_char(c) { '\u{00DF}' } else { c })
            .collect(),
    )
}

/// Pure Safe-Rust extractor for `.sh`, `.bash`, and extensionless shebang shell scripts.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShellExtractor;

impl LanguageExtractor for ShellExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let safe = sanitize_bash_scanner_ub(source);
        let src = safe.as_ref();
        let ts_lang = tree_sitter_bash::LANGUAGE.into();
        let tree = parse_with_language(&ts_lang, src)?;
        let root = tree.root_node();

        let (total_lines, effective_lines) = line_metrics(src);
        let mut doc =
            UastDocument::new(path.clone(), Language::Shell, total_lines, effective_lines)?;
        doc.syntax_errors = collect_syntax_errors(root);

        let dialect = detect_shell_dialect(path, src);
        let strict_flags = scan_top_level_strict_flags(root, src);
        if !is_sourced_shell_library(path, src) {
            check_strict_mode_header(dialect, strict_flags, &mut doc.lang_rule_findings);
        }

        let mut uast_children = Vec::new();
        collect_shell_uast_metrics(root, src, &mut doc, &mut uast_children);
        check_unquoted_expansions(root, src, &mut doc.lang_rule_findings);
        check_shell_hazards(
            root,
            src,
            strict_flags.has_errexit(),
            &mut doc.lang_rule_findings,
        );
        if dialect == ShellDialect::PosixSh {
            check_posix_bashisms(root, src, &mut doc.lang_rule_findings);
        }

        populate_cst_fortress(root, src, &mut doc);
        doc.root.children = uast_children;
        Ok(doc)
    }
}

/// Output of analyzing an embedded `Dockerfile RUN` or YAML shell snippet.
pub(crate) struct BridgedShellRun {
    /// Tier 1 Shell AST findings (`SH-SC2086`, `SH-SC2046`, `SH-SC2155`, `SH-SC2115`, `SH-SC2164`, `SH-SC3000`, `SH-NO-EVAL`).
    pub findings: Vec<LangRuleFinding>,
    /// Whether the snippet contains a CST `pipeline` `|` without `set -o pipefail`.
    pub has_unguarded_pipe: bool,
}

/// Analyzes an embedded shell snippet (from `Dockerfile RUN`, Compose `sh -c`, or GitHub Actions `run:`).
///
/// # Errors
///
/// Returns [`ParseError`] if configuring `tree-sitter-bash` fails.
pub(crate) fn analyze_embedded_shell_snippet(
    dialect: ShellDialect,
    script: &str,
    default_errexit: bool,
) -> Result<BridgedShellRun, ParseError> {
    let safe = sanitize_bash_scanner_ub(script);
    let src = safe.as_ref();
    let ts_lang = tree_sitter_bash::LANGUAGE.into();
    let tree = parse_with_language(&ts_lang, src)?;
    let root = tree.root_node();

    let strict_flags = scan_top_level_strict_flags(root, src);
    let errexit = default_errexit || strict_flags.has_errexit();
    let mut findings = Vec::new();
    check_unquoted_expansions(root, src, &mut findings);
    check_shell_hazards(root, src, errexit, &mut findings);
    if dialect == ShellDialect::PosixSh {
        check_posix_bashisms(root, src, &mut findings);
    }
    let has_unguarded_pipe = has_unguarded_shell_pipeline(root, src);
    Ok(BridgedShellRun {
        findings,
        has_unguarded_pipe,
    })
}

/// Analyzes an embedded `Dockerfile RUN` snippet under `dialect` (`default_errexit = false`).
///
/// # Errors
///
/// Returns [`ParseError`] if configuring `tree-sitter-bash` fails.
pub(super) fn analyze_dockerfile_shell_run(
    dialect: ShellDialect,
    script: &str,
) -> Result<BridgedShellRun, ParseError> {
    analyze_embedded_shell_snippet(dialect, script, false)
}
