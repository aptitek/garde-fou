//! Pure `#![no_std]` evaluators for native secret detection (`GF040`),
//! `LCOV` line/branch coverage (`GF050`), and callable `CRAP` scores (`GF051`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    CoverageConfig, Diagnostic, FileCoverage, LcovSummary, RepoPath, Severity, Span, UastDocument,
    Validation,
};

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

fn check_branch_threshold(
    cov: &LcovSummary,
    required_branch_pct: u32,
    report_path: &RepoPath,
    out: &mut Vec<Diagnostic>,
) {
    let Some(actual_pct) = cov.branch_coverage_pct() else {
        out.push(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "coverage report `{report_path}` contains zero instrumented `BRDA`/`BRF` branches (required >= {required_branch_pct}%)"
            ),
            remediation: String::from(
                "Enable branch instrumentation in your coverage runner (e.g. `--branch`) so `lcov.info` records `BRDA:` branch execution.",
            ),
            measured: 0,
            threshold: required_branch_pct,
            path: report_path.clone(),
            span: Span::FIRST,
        });
        return;
    };
    if actual_pct < required_branch_pct {
        out.push(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "branch coverage is {actual_pct}% ({}/{} branches), below required {required_branch_pct}%",
                cov.total_branches_hit, cov.total_branches_found
            ),
            remediation: String::from(
                "Add unit or property-based tests exercising both `true` and `false` / `Err` decision arms of conditional branches.",
            ),
            measured: actual_pct,
            threshold: required_branch_pct,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    }
}

fn check_line_threshold(
    cov: &LcovSummary,
    required_line: u32,
    report_path: &RepoPath,
    out: &mut Vec<Diagnostic>,
) {
    let actual_line = cov.line_coverage_pct();
    if actual_line < required_line {
        out.push(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "line coverage is {actual_line}% ({}/{} lines), below required {required_line}%",
                cov.total_lines_hit, cov.total_lines_found
            ),
            remediation: String::from(
                "Add meaningful unit, integration, or property-based tests covering untested domain branches; never write assertion-free smoke calls solely to inflate coverage.",
            ),
            measured: actual_line,
            threshold: required_line,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    }
}

/// Evaluates a parsed [`LcovSummary`] against line and branch coverage thresholds (`GF050`).
#[must_use]
pub fn evaluate_lcov_coverage(
    summary: Option<&LcovSummary>,
    coverage: &CoverageConfig,
    report_path: &RepoPath,
) -> Validation<(), Diagnostic> {
    if coverage.min_line_coverage.is_none() && coverage.min_branch_coverage.is_none() {
        return Validation::Valid(());
    }
    let fallback_req = coverage
        .min_line_coverage
        .or(coverage.min_branch_coverage)
        .unwrap_or(0);

    let Some(cov) = summary.filter(|s| s.total_lines_found > 0 || s.total_branches_found > 0)
    else {
        return Validation::invalid_one(Diagnostic {
            rule_id: String::from("GF050"),
            severity: Severity::Error,
            message: format!(
                "coverage report `{report_path}` is missing or contains zero instrumented records (required >= {fallback_req}%)"
            ),
            remediation: String::from(
                "Generate an `lcov.info` report via your test runner (e.g. `cargo llvm-cov --lcov --output-path lcov.info`) before running coverage-gated checks.",
            ),
            measured: 0,
            threshold: fallback_req,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    };

    let mut diags = Vec::new();
    if let Some(required_line) = coverage.min_line_coverage {
        check_line_threshold(cov, required_line, report_path, &mut diags);
    }
    if let Some(required_branch) = coverage.min_branch_coverage {
        check_branch_threshold(cov, required_branch, report_path, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn find_file_coverage<'a>(cov: &'a LcovSummary, path: &RepoPath) -> Option<&'a FileCoverage> {
    if let Some(exact) = cov.files.get(path.as_str()) {
        return Some(exact);
    }
    let suffix = format!("/{path}");
    cov.files
        .iter()
        .find_map(|(k, v)| k.ends_with(&suffix).then_some(v))
}

const fn compute_crap_u128(cyclomatic: u32, cov_pct: u32) -> u128 {
    let c = cyclomatic as u128;
    let uncovered = 100_u32.saturating_sub(cov_pct) as u128;
    c.saturating_mul(c)
        .saturating_mul(uncovered)
        .saturating_mul(uncovered)
        .saturating_mul(uncovered)
        / 1_000_000
        + c
}

/// Evaluates per-callable `CRAP` complexity-risk scores against `max_crap` (`GF051`).
#[must_use]
pub fn evaluate_crap_scores(
    docs: &[UastDocument],
    summary: Option<&LcovSummary>,
    max_crap: Option<u32>,
) -> Validation<(), Diagnostic> {
    let (Some(threshold), Some(cov)) = (max_crap, summary) else {
        return Validation::Valid(());
    };
    let mut diags = Vec::new();

    for doc in docs {
        let Some(file_cov) = find_file_coverage(cov, &doc.path) else {
            continue;
        };
        for callable in &doc.callables {
            let Some(cov_pct) = file_cov.span_line_coverage_pct(callable.span) else {
                continue;
            };
            let crap = compute_crap_u128(callable.cyclomatic_complexity, cov_pct);
            if crap > u128::from(threshold) {
                let measured = u32::try_from(crap).unwrap_or(u32::MAX);
                diags.push(Diagnostic {
                    rule_id: String::from("GF051"),
                    severity: Severity::Error,
                    message: format!(
                        "callable `{}` in `{}` has CRAP score {crap} (cyclomatic={}, line_coverage={cov_pct}%, max {threshold})",
                        callable.name, doc.path, callable.cyclomatic_complexity
                    ),
                    remediation: String::from(
                        "Reduce cyclomatic complexity by extracting pure helpers or add targeted tests covering the callable's branches.",
                    ),
                    measured,
                    threshold,
                    path: doc.path.clone(),
                    span: callable.span,
                });
            }
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
