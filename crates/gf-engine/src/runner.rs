//! Parallel `rayon` file analysis, external `SARIF` / `LCOV` gates, and workspace aggregation.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use gf_core::{Config, Diagnostic, ProjectContext, RepoPath, UastDocument};
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};
use rayon::prelude::*;

use crate::scanners::run_external_gates;
use crate::{
    CacheKey, CachedFileArtifact, CheckOptions, CheckReport, DiskCache, EngineError, ScanScope,
    detect_project_context, discover_files,
};

struct FileOutcome {
    file_key: CacheKey,
    was_hit: bool,
    artifact: CachedFileArtifact,
}

fn load_or_analyze_file(
    root: &Path,
    repo_path: &RepoPath,
    ctx: (&Config, CacheKey, Option<&DiskCache>, bool),
) -> Result<FileOutcome, EngineError> {
    let (config, config_hash, disk_cache, bypass_read) = ctx;
    let source = fs::read_to_string(root.join(repo_path.as_str()))?;
    let content_hash = CacheKey::from_bytes(source.as_bytes());
    let file_key = CacheKey::for_file(
        env!("CARGO_PKG_VERSION"),
        config_hash,
        repo_path,
        content_hash,
    );

    if !bypass_read
        && let Some(cache) = disk_cache
        && let Some(artifact) = cache.get(file_key)
    {
        return Ok(FileOutcome {
            file_key,
            was_hit: true,
            artifact,
        });
    }

    let document = parse_source(repo_path, &source)?;
    let local_diagnostics = evaluate_document(&document, config);
    let artifact = CachedFileArtifact {
        document,
        local_diagnostics,
    };
    if let Some(cache) = disk_cache {
        let _ = cache.put(file_key, &artifact);
    }

    Ok(FileOutcome {
        file_key,
        was_hit: false,
        artifact,
    })
}

/// Executes file discovery, parallel `v2` cache/parse pass, workspace rules, and external gates.
pub fn execute_pipeline(
    root: &Path,
    options: &CheckOptions,
    config: &Config,
    disk_cache: Option<&DiskCache>,
) -> Result<CheckReport, EngineError> {
    let all_files = discover_files(root, ScanScope::All, config)?;
    let context = detect_project_context(root, &all_files, config);
    let mut external_diags = run_external_gates(root, options, config)?;

    let staged_filter: Option<BTreeSet<RepoPath>> = (options.scope == ScanScope::StagedOnly)
        .then(|| discover_files(root, ScanScope::StagedOnly, config))
        .transpose()?
        .map(|v| v.into_iter().collect());

    if let Some(staged) = &staged_filter
        && staged.is_empty()
    {
        external_diags.sort();
        return Ok(CheckReport {
            scope: options.scope,
            context,
            files_checked: 0,
            cache_hits: 0,
            cache_misses: 0,
            cache_fingerprint: CacheKey::from_bytes(config.canonical_string().as_bytes()).to_hex(),
            diagnostics: external_diags,
        });
    }

    let config_hash = CacheKey::from_bytes(config.canonical_string().as_bytes());
    let outcomes: Vec<FileOutcome> = all_files
        .par_iter()
        .map(|path| {
            let bypass = options.no_cache
                && staged_filter
                    .as_ref()
                    .is_none_or(|staged| staged.contains(path));
            load_or_analyze_file(root, path, (config, config_hash, disk_cache, bypass))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(assemble_report(
        (options.scope, context),
        config,
        outcomes,
        (staged_filter.as_ref(), external_diags),
    ))
}

fn assemble_report(
    header: (ScanScope, ProjectContext),
    config: &Config,
    outcomes: Vec<FileOutcome>,
    extras: (Option<&BTreeSet<RepoPath>>, Vec<Diagnostic>),
) -> CheckReport {
    let (scope, context) = header;
    let (staged_filter, external_diags) = extras;
    let mut fp_hasher = blake3::Hasher::new();
    fp_hasher.update(config.canonical_string().as_bytes());

    let mut cache_hits = 0_usize;
    let mut cache_misses = 0_usize;
    let mut docs: Vec<UastDocument> = Vec::with_capacity(outcomes.len());
    let mut diagnostics = external_diags;

    for outcome in outcomes {
        fp_hasher.update(outcome.file_key.to_hex().as_bytes());
        let in_scope = staged_filter.is_none_or(|s| s.contains(&outcome.artifact.document.path));
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

    for diag in evaluate_workspace_sorted(&docs, &context, config) {
        if staged_filter.is_none_or(|s| s.contains(&diag.path)) {
            diagnostics.push(diag);
        }
    }
    diagnostics.sort();

    CheckReport {
        scope,
        context,
        files_checked: staged_filter.map_or(docs.len(), BTreeSet::len),
        cache_hits,
        cache_misses,
        cache_fingerprint: fp_hasher.finalize().to_hex().to_string(),
        diagnostics,
    }
}
