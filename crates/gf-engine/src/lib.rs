//! Imperative shell orchestrating strict `TOML` config, `ignore`/Git discovery,
//! parallel `rayon` analysis, and `BLAKE3` on-disk caching.

pub mod cache;
pub mod config_loader;
pub mod discovery;
pub mod scaffold;

use core::fmt;
use std::fs;
use std::path::Path;

use gf_core::{Config, Diagnostic, RepoPath, RepoPathError};
use gf_parsers::{ParseError, parse_source};
use gf_rules::evaluate_document;
use rayon::prelude::*;

pub use cache::{CacheKey, DiskCache};
pub use config_loader::{LoadedWorkspaceConfig, load_workspace_config};
pub use discovery::discover_files;
pub use scaffold::{DEFAULT_CONFIG_FILENAME, InitOutcome, scaffold_config};

/// Scope of files selected for a check run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanScope {
    /// Analyze the full repository workspace.
    All,
    /// Analyze only staged Git changes (`--staged`).
    StagedOnly,
}

/// Execution flags controlling [`run_check_with_options`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckOptions {
    /// File selection scope (`All` vs `StagedOnly`).
    pub scope: ScanScope,
    /// Bypass cached diagnostics and force full `Tree-sitter` re-evaluation.
    pub no_cache: bool,
}

/// Summary report returned by [`run_check`] and [`run_check_with_options`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckReport {
    /// Requested scan scope.
    pub scope: ScanScope,
    /// Total number of source files analyzed during this run.
    pub files_checked: usize,
    /// Number of files served directly from the `BLAKE3` disk cache.
    pub cache_hits: usize,
    /// Number of files parsed and evaluated via `Tree-sitter` + `gf-rules`.
    pub cache_misses: usize,
    /// Hexadecimal `BLAKE3` fingerprint summarizing the analyzed workspace state.
    pub cache_fingerprint: String,
    /// Deterministically sorted diagnostics across all checked files.
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckReport {
    /// Returns `true` when zero diagnostics were emitted.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Operational errors returned by the `gf-engine` imperative shell.
#[derive(Debug)]
pub enum EngineError {
    /// Filesystem I/O error.
    Io(std::io::Error),
    /// Repository path validation failure.
    InvalidPath(RepoPathError),
    /// Strict `garde-fou.toml` or glob configuration error.
    InvalidConfig(String),
    /// External `git` command failure.
    GitCommand(String),
    /// Source-to-UAST parser failure.
    Parse(ParseError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidPath(err) => write!(f, "invalid repository path: {err}"),
            Self::InvalidConfig(msg) => write!(f, "configuration error in garde-fou.toml: {msg}"),
            Self::GitCommand(msg) => write!(f, "git command failed: {msg}"),
            Self::Parse(err) => write!(f, "parse error: {err}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<RepoPathError> for EngineError {
    fn from(err: RepoPathError) -> Self {
        Self::InvalidPath(err)
    }
}

impl From<ParseError> for EngineError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

struct FileOutcome {
    file_key: CacheKey,
    was_hit: bool,
    diagnostics: Vec<Diagnostic>,
}

fn analyze_one_file(
    root: &Path,
    repo_path: &RepoPath,
    ctx: (&Config, CacheKey, Option<&DiskCache>, bool),
) -> Result<FileOutcome, EngineError> {
    let (config, config_hash, disk_cache, no_cache) = ctx;
    let source = fs::read_to_string(root.join(repo_path.as_str()))?;
    let content_hash = CacheKey::from_bytes(source.as_bytes());
    let file_key = CacheKey::for_file(
        env!("CARGO_PKG_VERSION"),
        config_hash,
        repo_path,
        content_hash,
    );

    if !no_cache
        && let Some(cache) = disk_cache
        && let Some(cached) = cache.get(file_key)
    {
        return Ok(FileOutcome {
            file_key,
            was_hit: true,
            diagnostics: cached,
        });
    }

    let uast = parse_source(repo_path, &source)?;
    let diagnostics = evaluate_document(&uast, config);
    if let Some(cache) = disk_cache {
        let _ = cache.put(file_key, &diagnostics);
    }

    Ok(FileOutcome {
        file_key,
        was_hit: false,
        diagnostics,
    })
}

fn execute_pipeline(
    root: &Path,
    options: CheckOptions,
    config: &Config,
    disk_cache: Option<&DiskCache>,
) -> Result<CheckReport, EngineError> {
    let files = discover_files(root, options.scope, config)?;
    let config_hash = CacheKey::from_bytes(config.canonical_string().as_bytes());

    let outcomes: Vec<FileOutcome> = files
        .par_iter()
        .map(|repo_path| {
            analyze_one_file(
                root,
                repo_path,
                (config, config_hash, disk_cache, options.no_cache),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut fp_hasher = blake3::Hasher::new();
    fp_hasher.update(config.canonical_string().as_bytes());
    let mut cache_hits = 0_usize;
    let mut cache_misses = 0_usize;
    let mut diagnostics = Vec::new();

    for outcome in outcomes {
        fp_hasher.update(outcome.file_key.to_hex().as_bytes());
        if outcome.was_hit {
            cache_hits = cache_hits.saturating_add(1);
        } else {
            cache_misses = cache_misses.saturating_add(1);
        }
        diagnostics.extend(outcome.diagnostics);
    }
    diagnostics.sort();

    Ok(CheckReport {
        scope: options.scope,
        files_checked: files.len(),
        cache_hits,
        cache_misses,
        cache_fingerprint: fp_hasher.finalize().to_hex().to_string(),
        diagnostics,
    })
}

/// Loads `garde-fou.toml` from `root` and executes parallel analysis with optional `BLAKE3` caching.
///
/// # Errors
///
/// Returns [`EngineError`] if configuration loading, discovery, I/O, or parsing fails.
pub fn run_check_with_options(
    root: &Path,
    options: CheckOptions,
) -> Result<CheckReport, EngineError> {
    let loaded = load_workspace_config(root)?;
    let cache = loaded
        .cache_enabled
        .then(|| DiskCache::new(root, &loaded.cache_dir));
    execute_pipeline(root, options, &loaded.analyzer, cache.as_ref())
}

/// Runs analysis on `root` using an explicit in-memory [`Config`] without disk caching.
///
/// # Errors
///
/// Returns [`EngineError`] if file discovery, I/O, or parsing fails.
pub fn run_check(
    root: &Path,
    scope: ScanScope,
    config: &Config,
) -> Result<CheckReport, EngineError> {
    execute_pipeline(
        root,
        CheckOptions {
            scope,
            no_cache: true,
        },
        config,
        None,
    )
}
