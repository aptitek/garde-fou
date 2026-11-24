//! Polyglot mutation-report ingestion (`cargo-mutants` `outcomes.json` & `Stryker`
//! `mutation.json`) plus Git committed-artifact anti-fraud verification (`GF052` / `GF070`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use gf_core::{
    Config, Diagnostic, MutationConfig, MutationSummary, RepoPath, Severity, Span, SurvivedMutant,
    Validation, truncate_replacement,
};
use gf_rules::evaluate_mutation_summary;

use super::mutation_schema::{
    CargoMutantDetail, CargoMutantOutcome, CargoMutantScenario, CargoMutantsReport,
    NormalizedRawSurvivor, StrykerFileEntry, StrykerMutant, StrykerReport,
};
use super::sarif_ingest::normalize_artifact_uri;
use crate::types::EngineError;

const WELL_KNOWN_MUTATION_REPORTS: &[&str] = &[
    "outcomes.json",
    "mutants.out/outcomes.json",
    "mutants.out/mutants.out/outcomes.json",
    "mutation.json",
    "reports/mutation/mutation.json",
];

fn build_survived_mutant(
    norm: NormalizedRawSurvivor,
    workspace_root: &Path,
) -> Result<SurvivedMutant, EngineError> {
    let path = normalize_artifact_uri(norm.raw_uri.as_deref(), workspace_root, norm.fallback_path)?;
    let raw_rep = norm.replacement.as_deref().unwrap_or("<mutated>");
    Ok(SurvivedMutant {
        path,
        span: norm.span,
        function_name: norm.function_name,
        mutator: norm.mutator,
        replacement: truncate_replacement(raw_rep),
    })
}

fn ingest_cargo_mutants(
    val: serde_json::Value,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let report: CargoMutantsReport = serde_json::from_value(val)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid cargo-mutants JSON: {e}")))?;
    let mut out = MutationSummary::default();

    for CargoMutantOutcome { summary, scenario } in report.outcomes {
        let Some(CargoMutantScenario::Tagged { mutant }) = scenario else {
            continue;
        };
        match summary.as_deref() {
            Some("CaughtMutant") => out.caught = out.caught.saturating_add(1),
            Some("Timeout") => out.timeout = out.timeout.saturating_add(1),
            Some("Unviable") => out.unviable = out.unviable.saturating_add(1),
            Some("MissedMutant") => {
                out.missed = out.missed.saturating_add(1);
                let detail: CargoMutantDetail = *mutant;
                out.survivors.push(build_survived_mutant(
                    detail.into_normalized(),
                    workspace_root,
                )?);
            }
            _ => {}
        }
    }
    out.survivors.sort();
    Ok(out)
}

fn ingest_stryker_elements(
    val: serde_json::Value,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let report: StrykerReport = serde_json::from_value(val)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid mutation.json report: {e}")))?;
    let mut out = MutationSummary::default();

    for (file_key, StrykerFileEntry { mutants }) in report.files {
        for m in mutants {
            match m.status.as_deref() {
                Some("Killed") => out.caught = out.caught.saturating_add(1),
                Some("Timeout") => out.timeout = out.timeout.saturating_add(1),
                Some("CompileError" | "Ignored") => out.unviable = out.unviable.saturating_add(1),
                Some("Survived" | "NoCoverage" | "RuntimeError") => {
                    out.missed = out.missed.saturating_add(1);
                    let mutant: StrykerMutant = m;
                    out.survivors.push(build_survived_mutant(
                        mutant.into_normalized(&file_key),
                        workspace_root,
                    )?);
                }
                _ => {}
            }
        }
    }
    out.survivors.sort();
    Ok(out)
}

/// Auto-detects and parses `cargo-mutants` (`outcomes.json`) or `Stryker` (`mutation.json`).
///
/// # Errors
///
/// Returns [`EngineError::InvalidConfig`] if `raw_json` is malformed or matches neither schema.
pub fn ingest_mutation_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let val: serde_json::Value = serde_json::from_str(raw_json)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid mutation JSON: {e}")))?;
    if val.get("outcomes").is_some() {
        ingest_cargo_mutants(val, workspace_root)
    } else if val.get("files").is_some() {
        ingest_stryker_elements(val, workspace_root)
    } else {
        Err(EngineError::InvalidConfig(String::from(
            "unrecognized mutation report schema (expected `outcomes` or `files` root key)",
        )))
    }
}

fn is_tracked_in_git(root: &Path, rel_path: &str) -> bool {
    root.join(".git").exists()
        && Command::new("git")
            .args(["ls-files", "--error-unmatch", "--", rel_path])
            .current_dir(root)
            .output()
            .is_ok_and(|out| out.status.success())
}

fn check_committed_mutation_reports(
    root: &Path,
    explicit_rel: Option<&str>,
) -> Result<Vec<Diagnostic>, EngineError> {
    let mut candidates: BTreeSet<&str> = WELL_KNOWN_MUTATION_REPORTS.iter().copied().collect();
    if let Some(rel) = explicit_rel {
        candidates.insert(rel);
    }
    let mut diags = Vec::new();
    for rel in candidates {
        if root.join(rel).is_file() && is_tracked_in_git(root, rel) {
            diags.push(Diagnostic {
                rule_id: String::from("GF070"),
                severity: Severity::Error,
                message: format!(
                    "committed mutation report artifact `{rel}` is forbidden; generate fresh mutation results in CI/local runs"
                ),
                remediation: String::from(
                    "Remove the tracked mutation report from Git (`git rm --cached <report>`) and keep `mutants.out*/` and `reports/mutation/` in `.gitignore`.",
                ),
                measured: 1,
                threshold: 0,
                path: RepoPath::new(rel)?,
                span: Span::FIRST,
            });
        }
    }
    Ok(diags)
}

fn resolve_default_report_rel(root: &Path) -> &'static str {
    if !root.join("mutants.out/outcomes.json").is_file()
        && root.join("reports/mutation/mutation.json").is_file()
    {
        "reports/mutation/mutation.json"
    } else {
        "mutants.out/outcomes.json"
    }
}

pub(super) fn evaluate_workspace_mutation(
    root: &Path,
    config: &Config,
    mutants_override: Option<&str>,
) -> Result<Vec<Diagnostic>, EngineError> {
    let mut_cfg = &config.mutation;
    let explicit_rel = mutants_override.or(mut_cfg.report_path.as_deref());
    let mut out = check_committed_mutation_reports(root, explicit_rel)?;

    let has_explicit =
        mut_cfg.min_mutation_score.is_some() || !mut_cfg.zero_survivor_globs.is_empty();
    let default_cli_score = (!has_explicit && mutants_override.is_some()).then_some(85);
    let effective = MutationConfig {
        min_mutation_score: mut_cfg.min_mutation_score.or(default_cli_score),
        zero_survivor_globs: mut_cfg.zero_survivor_globs.clone(),
        report_path: mut_cfg.report_path.clone(),
    };
    if effective.min_mutation_score.is_none() && effective.zero_survivor_globs.is_empty() {
        return Ok(out);
    }

    let rel_str = explicit_rel.unwrap_or_else(|| resolve_default_report_rel(root));
    let repo_path = RepoPath::new(rel_str)?;
    let primary = root.join(repo_path.as_str());
    let nested_fallback = primary
        .parent()
        .map(|p| p.join("mutants.out/outcomes.json"))
        .filter(|p| p.is_file());
    let resolved_file = primary.is_file().then_some(primary).or(nested_fallback);
    let parsed = if let Some(actual_file) = resolved_file {
        let raw = fs::read_to_string(actual_file)?;
        Some(ingest_mutation_json(&raw, root)?)
    } else {
        None
    };

    if let Validation::Invalid(diags) =
        evaluate_mutation_summary(parsed.as_ref(), &effective, &repo_path)
    {
        out.extend(diags);
    }
    Ok(out)
}
