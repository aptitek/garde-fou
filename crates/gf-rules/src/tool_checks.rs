//! Pure `#![no_std]` evaluators for Tier 1 (`GF080`), Tier 2 (`GF081`), and Tier 3 (`GF082`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, ExternalToolFinding, PytestCoverageIssue, Span, ToolConfigViolation,
    UastDocument, Validation, glob_matches,
};

use crate::checks::{SourceLoc, rule_error};

fn is_declared_cli_entrypoint(path_str: &str, config: &Config) -> bool {
    config
        .entrypoints
        .iter()
        .any(|pat| glob_matches(pat, path_str))
}

/// Evaluates Tier 1 language AST findings (`GF080`) and Tier 2 tool config findings (`GF081`).
pub fn check_local_tool_and_lang_rules(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    let is_cli_entry = is_declared_cli_entrypoint(doc.path.as_str(), config);

    for finding in &doc.lang_rule_findings {
        if finding.violation.is_entrypoint_exempt() && is_cli_entry {
            continue;
        }
        diags.push(rule_error(
            "GF080",
            format!("[{}] {}", finding.code(), finding.message),
            finding.remediation(),
            SourceLoc::new(&doc.path, finding.span),
        ));
    }

    for cfg_finding in &doc.tool_config_findings {
        diags.push(rule_error(
            "GF081",
            format!("[{}] {}", cfg_finding.violation, cfg_finding.message),
            cfg_finding.remediation(),
            SourceLoc::new(&doc.path, cfg_finding.span),
        ));
    }

    if let (Some(min_cov), Some(decl)) =
        (config.coverage.min_line_coverage, doc.coverage_fail_under)
        && decl.floor_pct < min_cov
    {
        let v = ToolConfigViolation::PytestOrCoverageWeakened(
            PytestCoverageIssue::CoverageFailUnderTooLow,
        );
        diags.push(rule_error(
            "GF081",
            format!(
                "[{v}] `[tool.coverage.report] fail_under = {}` is below required `min_line_coverage = {min_cov}`",
                decl.floor_pct
            ),
            v.remediation(),
            SourceLoc::new(&doc.path, decl.span),
        ));
    }

    Validation::<(), Diagnostic>::from_errors(diags)
}

/// Pure deterministic converter turning [`ExternalToolFinding`] items into sorted,
/// deduplicated `GF082` [`Diagnostic`]s.
#[must_use]
pub fn convert_external_tool_findings(findings: &[ExternalToolFinding]) -> Vec<Diagnostic> {
    let unique: BTreeSet<&ExternalToolFinding> = findings.iter().collect();
    let mut out: Vec<Diagnostic> = unique
        .into_iter()
        .map(|f| {
            let line = f.line.max(1);
            let col = f.column.max(1);
            let span = Span::new(line, line).unwrap_or(Span::FIRST);
            let code = if f.native_code.trim().is_empty() {
                "error"
            } else {
                f.native_code.trim()
            };
            rule_error(
                "GF082",
                format!("[{}:{code}] {} (col {col})", f.tool, f.message.trim()),
                f.remediation.trim(),
                SourceLoc::new(&f.path, span),
            )
        })
        .collect();
    out.sort();
    out.dedup();
    out
}
