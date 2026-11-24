//! Parallel `rayon` workspace check pipeline, external `SARIF` / `LCOV` / `CRAP` gates,
//! and deterministic diagnostic aggregation.

use std::collections::BTreeSet;
use std::path::Path;

use gf_core::{Config, Diagnostic, LockStatus, ProjectContext, RepoPath, UastDocument, Validation};
use gf_rules::{check_agentic_governance, check_lock_integrity, evaluate_workspace_sorted};
use rayon::prelude::*;

use crate::adapters::{AdapterCtx, run_language_tool_adapters};
use crate::agents::verify_agentic_state;
use crate::cache::{CacheKey, DiskCache};
use crate::context_detector::detect_project_context;
use crate::discovery::discover_files;
use crate::file_loader::{FileOutcome, FileScanCtx, load_or_analyze_file};
use crate::scanners::run_external_gates;
use crate::types::{CheckOptions, CheckReport, EngineError, ScanScope};

/// Execution environment shared across the workspace check pipeline (zero tuple-packing).
pub struct PipelineEnv<'a> {
    /// Repository root directory.
    pub root: &'a Path,
    /// CLI/invocation check options.
    pub options: &'a CheckOptions,
    /// Validated pure analyzer configuration.
    pub config: &'a Config,
    /// Optional content-addressed `v6` disk cache.
    pub disk_cache: Option<&'a DiskCache>,
    /// Cryptographic `garde.lock` status (`GF070`).
    pub lock_status: &'a LockStatus,
    /// Whether `AGENTS.md` / embedded skill verification is enforced on this run.
    pub enforce_agentic: bool,
}

struct ReportInputs<'a> {
    scope: ScanScope,
    context: ProjectContext,
    config: &'a Config,
    staged_filter: Option<&'a BTreeSet<RepoPath>>,
}

fn collect_gate_diagnostics(
    env: &PipelineEnv<'_>,
    context: &ProjectContext,
    staged_filter: Option<&BTreeSet<RepoPath>>,
    docs: &[UastDocument],
) -> Result<Vec<Diagnostic>, EngineError> {
    let mut external_diags = run_external_gates(env.root, env.options, env.config, docs)?;
    let adapter_ctx = AdapterCtx {
        root: env.root,
        context,
        staged_filter,
        options: env.options,
        config: env.config,
    };
    external_diags.extend(run_language_tool_adapters(&adapter_ctx)?);
    if let Validation::Invalid(lock_diags) = check_lock_integrity(env.lock_status) {
        external_diags.extend(lock_diags);
    }
    if env.enforce_agentic {
        let agent_status = verify_agentic_state(env.root, context, env.config)?;
        if let Validation::Invalid(agent_diags) = check_agentic_governance(&agent_status) {
            external_diags.extend(agent_diags);
        }
    }
    Ok(external_diags)
}

/// Executes file discovery, parallel `v6` cache/parse pass, workspace rules, and external gates.
pub fn execute_pipeline(env: &PipelineEnv<'_>) -> Result<CheckReport, EngineError> {
    let all_files = discover_files(env.root, ScanScope::All, env.config)?;
    let context = detect_project_context(env.root, &all_files, env.config);
    let staged_filter: Option<BTreeSet<RepoPath>> = (env.options.scope == ScanScope::StagedOnly)
        .then(|| discover_files(env.root, ScanScope::StagedOnly, env.config))
        .transpose()?
        .map(|v| v.into_iter().collect());

    if staged_filter.as_ref().is_some_and(BTreeSet::is_empty) {
        let mut external_diags =
            collect_gate_diagnostics(env, &context, staged_filter.as_ref(), &[])?;
        external_diags.sort();
        return Ok(CheckReport {
            scope: env.options.scope,
            context,
            files_checked: 0,
            cache_hits: 0,
            cache_misses: 0,
            cache_fingerprint: CacheKey::from_bytes(env.config.canonical_string().as_bytes())
                .to_hex(),
            diagnostics: external_diags,
        });
    }

    let config_hash = CacheKey::from_bytes(env.config.canonical_string().as_bytes());
    let outcomes: Vec<FileOutcome> = all_files
        .par_iter()
        .map(|path| {
            let bypass_read = env.options.no_cache
                && staged_filter
                    .as_ref()
                    .is_none_or(|staged| staged.contains(path));
            let scan_ctx = FileScanCtx {
                config: env.config,
                config_hash,
                disk_cache: env.disk_cache,
                bypass_read,
            };
            load_or_analyze_file(env.root, path, &scan_ctx)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let inputs = ReportInputs {
        scope: env.options.scope,
        context,
        config: env.config,
        staged_filter: staged_filter.as_ref(),
    };
    assemble_report(env, inputs, outcomes)
}

fn assemble_report(
    env: &PipelineEnv<'_>,
    inputs: ReportInputs<'_>,
    outcomes: Vec<FileOutcome>,
) -> Result<CheckReport, EngineError> {
    let mut fp_hasher = blake3::Hasher::new();
    fp_hasher.update(inputs.config.canonical_string().as_bytes());

    let mut cache_hits = 0_usize;
    let mut cache_misses = 0_usize;
    let mut docs: Vec<UastDocument> = Vec::with_capacity(outcomes.len());
    let mut diagnostics = Vec::new();

    for outcome in outcomes {
        fp_hasher.update(outcome.file_key.to_hex().as_bytes());
        let in_scope = inputs
            .staged_filter
            .is_none_or(|s| s.contains(&outcome.artifact.document.path));
        if in_scope {
            if outcome.was_hit {
                cache_hits = cache_hits.saturating_add(1);
            } else {
                cache_misses = cache_misses.saturating_add(1);
            }
            diagnostics.extend(outcome.artifact.local_diagnostics);
        }
        docs.push(outcome.artifact.document);
    }

    diagnostics.extend(collect_gate_diagnostics(
        env,
        &inputs.context,
        inputs.staged_filter,
        &docs,
    )?);

    for diag in evaluate_workspace_sorted(&docs, &inputs.context, inputs.config) {
        if inputs.staged_filter.is_none_or(|s| s.contains(&diag.path)) {
            diagnostics.push(diag);
        }
    }
    diagnostics.sort();

    Ok(CheckReport {
        scope: inputs.scope,
        context: inputs.context,
        files_checked: inputs.staged_filter.map_or(docs.len(), BTreeSet::len),
        cache_hits,
        cache_misses,
        cache_fingerprint: fp_hasher.finalize().to_hex().to_string(),
        diagnostics,
    })
}
