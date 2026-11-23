//! External `SARIF` scanner orchestration (`gitleaks`, `osv-scanner`) and `LCOV` gate (`GF050`).

pub mod gitleaks;
pub mod osv;
pub mod sarif_ingest;
mod sarif_schema;

use std::fs;
use std::path::Path;

use gf_core::{Config, Diagnostic, RepoPath, ScannerPolicy, Validation, parse_lcov};
use gf_rules::evaluate_lcov_coverage;

use crate::{CheckOptions, EngineError, ScanScope};

fn effective_policy(configured: ScannerPolicy, strict_tools: bool) -> ScannerPolicy {
    if strict_tools && configured == ScannerPolicy::Auto {
        ScannerPolicy::Required
    } else {
        configured
    }
}

fn evaluate_workspace_lcov(
    root: &Path,
    config: &Config,
    lcov_override: Option<&str>,
) -> Result<Vec<Diagnostic>, EngineError> {
    let min_pct = config.coverage.min_line_coverage;
    if min_pct.is_none() && lcov_override.is_none() {
        return Ok(Vec::new());
    }
    let rel_str = lcov_override
        .or(config.coverage.lcov_path.as_deref())
        .unwrap_or("lcov.info");
    let repo_path = RepoPath::new(rel_str)?;
    let full_path = root.join(repo_path.as_str());

    let parsed = fs::read_to_string(&full_path)
        .ok()
        .map(|raw| parse_lcov(&raw));
    let effective_min = min_pct.or_else(|| lcov_override.map(|_| 85));

    match evaluate_lcov_coverage(parsed.as_ref(), effective_min, &repo_path) {
        Validation::Valid(()) => Ok(Vec::new()),
        Validation::Invalid(diags) => Ok(diags),
    }
}

/// Runs external `SARIF` scanners in parallel plus optional `LCOV` coverage verification.
///
/// # Errors
///
/// Returns [`EngineError`] if subprocess execution, path validation, or SARIF ingestion fails.
pub fn run_external_gates(
    root: &Path,
    options: &CheckOptions,
    config: &Config,
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
        out.extend(evaluate_workspace_lcov(
            root,
            config,
            options.lcov_path.as_deref(),
        )?);
    }
    Ok(out)
}
