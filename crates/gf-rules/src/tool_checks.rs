//! Pure `#![no_std]` evaluators for Tier 1 (`GF080`), Tier 2 (`GF081`), and Tier 3 (`GF082`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, ExternalToolFinding, PytestCoverageIssue, Span, ToolConfigViolation,
    TsconfigIssue, TsconfigStrictState, TsconfigSummary, UastDocument, Validation, glob_matches,
    is_known_strict_preset,
};

use crate::checks::{SourceLoc, rule_error};
use crate::rust_import_resolver::{dir_of, normalize_join};

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

fn resolve_parent_tsconfig<'a>(
    cur_dir: &str,
    ext: &str,
    map: &'a BTreeMap<&'a str, &'a TsconfigSummary>,
) -> Option<(&'a str, &'a TsconfigSummary)> {
    let joined = normalize_join(cur_dir, ext);
    let candidates = [
        joined.clone(),
        format!("{joined}.json"),
        format!("{joined}/tsconfig.json"),
    ];
    for cand in &candidates {
        if let Some((&k, &sum)) = map.get_key_value(cand.as_str()) {
            return Some((k, sum));
        }
    }
    None
}

fn inherits_strict_from_extends(
    doc_path: &str,
    summary: &TsconfigSummary,
    map: &BTreeMap<&str, &TsconfigSummary>,
) -> bool {
    let mut visited = BTreeSet::new();
    visited.insert(String::from(doc_path));
    let mut cur_dir = dir_of(doc_path);
    let mut cur_ext = summary.extends.as_deref();

    for _ in 0..16 {
        let Some(ext) = cur_ext else {
            return false;
        };
        if is_known_strict_preset(ext) {
            return true;
        }
        let Some((parent_path, parent_sum)) = resolve_parent_tsconfig(cur_dir, ext, map) else {
            return false;
        };
        if !visited.insert(String::from(parent_path)) {
            return false;
        }
        match parent_sum.strict {
            TsconfigStrictState::ExplicitTrue => return true,
            TsconfigStrictState::ExplicitFalse => return false,
            TsconfigStrictState::Omitted => {
                cur_dir = dir_of(parent_path);
                cur_ext = parent_sum.extends.as_deref();
            }
        }
    }
    false
}

/// Verifies `tsconfig*.json` `"extends"` chains across workspace documents (`GF081`).
pub fn check_workspace_tsconfigs(docs: &[UastDocument]) -> Validation<(), Diagnostic> {
    let map: BTreeMap<&str, &TsconfigSummary> = docs
        .iter()
        .filter_map(|d| d.tsconfig.as_ref().map(|ts| (d.path.as_str(), ts)))
        .collect();
    let mut diags = Vec::new();
    for doc in docs {
        let Some(ts) = &doc.tsconfig else {
            continue;
        };
        if ts.strict == TsconfigStrictState::Omitted
            && ts.extends.is_some()
            && !inherits_strict_from_extends(doc.path.as_str(), ts, &map)
        {
            let v = ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict);
            diags.push(rule_error(
                "GF081",
                format!(
                    "[{v}] `{}` omits `\"strict\": true` and its `\"extends\"` chain does not enable strict mode",
                    doc.path
                ),
                v.remediation(),
                SourceLoc::new(&doc.path, ts.strict_span),
            ));
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
