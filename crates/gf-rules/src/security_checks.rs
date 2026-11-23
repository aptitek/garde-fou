//! Pure `#![no_std]` evaluators for native secret detection (`GF040`) and `LCOV` coverage (`GF050`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{Diagnostic, LcovSummary, RepoPath, Severity, Span, UastDocument, Validation};

/// Emits blocking `GF040` diagnostics for every redacted [`gf_core::SecretFinding`] in `doc`.
pub fn check_native_secrets(doc: &UastDocument) -> Validation<(), Diagnostic> {
    let diags: Vec<Diagnostic> = doc
        .secret_findings
        .iter()
        .map(|finding| Diagnostic {
            rule_id: String::from("GF040"),
            severity: Severity::Error,
            message: format!(
                "hardcoded secret detected ({}) matching `{}`",
                finding.kind, finding.redacted_preview
            ),
            remediation: String::from(
                "Revoke/rotate the exposed credential immediately and load secrets at runtime via environment variables or a secret manager; never hardcode tokens or private keys.",
            ),
            measured: 1,
            threshold: 0,
            path: doc.path.clone(),
            span: finding.span,
        })
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}

/// Evaluates a parsed [`LcovSummary`] against an optional minimum line-coverage threshold (`GF050`).
#[must_use]
pub fn evaluate_lcov_coverage(
    summary: Option<&LcovSummary>,
    min_line_coverage: Option<u32>,
    report_path: &RepoPath,
) -> Validation<(), Diagnostic> {
    let Some(required_pct) = min_line_coverage else {
        return Validation::Valid(());
    };

    let Some(cov) = summary.filter(|s| s.total_lines_found > 0) else {
        return Validation::invalid_one(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "coverage report `{report_path}` is missing or contains zero instrumented lines (required >= {required_pct}%)"
            ),
            remediation: String::from(
                "Generate an `lcov.info` report via your test runner (e.g. `cargo llvm-cov --lcov --output-path lcov.info`) before running coverage-gated checks.",
            ),
            measured: 0,
            threshold: required_pct,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    };

    let actual_pct = cov.line_coverage_pct();
    if actual_pct < required_pct {
        return Validation::invalid_one(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "line coverage is {actual_pct}% ({}/{} lines), below required {required_pct}%",
                cov.total_lines_hit, cov.total_lines_found
            ),
            remediation: String::from(
                "Add meaningful unit, integration, or property-based tests covering untested domain branches; never write assertion-free smoke calls solely to inflate coverage.",
            ),
            measured: actual_pct,
            threshold: required_pct,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    }

    Validation::Valid(())
}
