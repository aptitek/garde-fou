//! External `SARIF` scanner orchestration (`gitleaks`, `osv-scanner`) and `LCOV` / `CRAP`
//! gates (`GF050`–`GF051`).

pub mod gitleaks;
pub mod mutation_ingest;
mod mutation_schema;
pub mod osv;
pub mod sarif_ingest;
mod sarif_schema;

use std::fs;
use std::path::Path;

use gf_core::{
    Config, CoverageConfig, Diagnostic, RepoPath, ScannerPolicy, Severity, Span, UastDocument,
    Validation, parse_lcov,
};
use gf_rules::{evaluate_crap_scores, evaluate_lcov_coverage};

use crate::types::{CheckOptions, EngineError, ScanScope};

fn effective_policy(configured: ScannerPolicy, strict_tools: bool) -> ScannerPolicy {
    if strict_tools && configured == ScannerPolicy::Auto {
        ScannerPolicy::Required
    } else {
        configured
    }
}

fn missing_crap_lcov_diagnostic(repo_path: RepoPath, max_crap: u32) -> Diagnostic {
    Diagnostic {
        rule_id: String::from("GF051"),
        severity: Severity::Error,
        message: format!(
            "LCOV report `{repo_path}` is missing while `[coverage] max_crap_score = {max_crap}` is active"
        ),
        remediation: String::from(
            "Generate an `lcov.info` report before evaluating `max_crap_score` (`GF051`).",
        ),
        measured: max_crap.saturating_add(1),
        threshold: max_crap,
        path: repo_path,
        span: Span::FIRST,
    }
}

fn build_effective_coverage(cov: &CoverageConfig, lcov_override: Option<&str>) -> CoverageConfig {
    let has_explicit = cov.min_line_coverage.is_some()
        || cov.min_branch_coverage.is_some()
        || cov.max_crap_score.is_some();
    let default_cli_line = (!has_explicit && lcov_override.is_some()).then_some(85);
    CoverageConfig {
        min_line_coverage: cov.min_line_coverage.or(default_cli_line),
        min_branch_coverage: cov.min_branch_coverage,
        max_crap_score: cov.max_crap_score,
        lcov_path: cov.lcov_path.clone(),
    }
}

fn evaluate_workspace_lcov_and_crap(
    root: &Path,
    config: &Config,
    lcov_override: Option<&str>,
    docs: &[UastDocument],
) -> Result<Vec<Diagnostic>, EngineError> {
    let effective_cov = build_effective_coverage(&config.coverage, lcov_override);
    let has_gate = effective_cov.min_line_coverage.is_some()
        || effective_cov.min_branch_coverage.is_some()
        || effective_cov.max_crap_score.is_some();
    if !has_gate {
        return Ok(Vec::new());
    }
    let rel_str = lcov_override
        .or(effective_cov.lcov_path.as_deref())
        .unwrap_or("lcov.info");
    let repo_path = RepoPath::new(rel_str)?;
    let parsed = fs::read_to_string(root.join(repo_path.as_str()))
        .ok()
        .map(|raw| parse_lcov(&raw));

    let mut out = Vec::new();
    if let Validation::Invalid(diags) =
        evaluate_lcov_coverage(parsed.as_ref(), &effective_cov, &repo_path)
    {
        out.extend(diags);
    }
    let line_or_branch_active =
        effective_cov.min_line_coverage.is_some() || effective_cov.min_branch_coverage.is_some();
    if let Some(max_crap) = effective_cov.max_crap_score
        && parsed.is_none()
        && !line_or_branch_active
    {
        out.push(missing_crap_lcov_diagnostic(repo_path, max_crap));
    } else if let Validation::Invalid(diags) =
        evaluate_crap_scores(docs, parsed.as_ref(), effective_cov.max_crap_score)
    {
        out.extend(diags);
    }
    Ok(out)
}

/// Runs external `SARIF` scanners in parallel plus optional `LCOV` & `CRAP` verification.
///
/// # Errors
///
/// Returns [`EngineError`] if subprocess execution, path validation, or SARIF ingestion fails.
pub fn run_external_gates(
    root: &Path,
    options: &CheckOptions,
    config: &Config,
    docs: &[UastDocument],
) -> Result<Vec<Diagnostic>, EngineError> {
    let secrets_pol = effective_policy(config.scanners.secrets, options.strict_tools);
    let vulns_pol = effective_policy(config.scanners.vulns, options.strict_tools);

    let (secrets_res, vulns_res) = rayon::join(
        || gitleaks::run_gitleaks(root, options.scope, secrets_pol),
        || osv::run_osv_scanner(root, options.scope, vulns_pol),
    );

    let mut out = secrets_res?;
    out.extend(vulns_res?);
    if options.scope == ScanScope::All || options.lcov_path.is_some() {
        out.extend(evaluate_workspace_lcov_and_crap(
            root,
            config,
            options.lcov_path.as_deref(),
            docs,
        )?);
    }
    if options.scope == ScanScope::All || options.mutants_path.is_some() {
        out.extend(mutation_ingest::evaluate_workspace_mutation(
            root,
            config,
            options.mutants_path.as_deref(),
        )?);
    }
    Ok(out)
}
