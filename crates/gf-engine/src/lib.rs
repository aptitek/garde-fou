//! Imperative shell orchestrating strict `TOML` config, `garde lock`, `uv`-style `init`,
//! parallel `rayon` analysis, `.garde-fou-cache/v4/` caching, and `SARIF` / `LCOV` gates.

pub mod cache;
mod config_fortress;
pub mod config_loader;
mod config_schema;
pub mod context_detector;
pub mod discovery;
pub mod init;
pub mod lockfile;
mod runner;
pub mod scaffold;
pub mod scanners;
pub mod types;

use std::path::Path;

use gf_core::{Config, LockStatus};

pub use cache::{CacheKey, CachedFileArtifact, DiskCache};
pub use config_loader::{LoadedWorkspaceConfig, load_workspace_config};
pub use context_detector::detect_project_context;
pub use discovery::discover_files;
pub use init::{InitReport, InitRequest, run_init};
pub use lockfile::{compute_manifest_blake3_hex, verify_garde_lock, write_garde_lock};
use runner::execute_pipeline;
pub use scaffold::{InitOutcome, scaffold_config};
pub use scanners::sarif_ingest::{SarifMapping, ingest_sarif_json};
pub use types::{
    CheckOptions, CheckReport, DEFAULT_CONFIG_FILENAME, EngineError, LOCK_FILENAME, ScanScope,
};

/// Validates `garde-fou.toml` in `root` and writes a cryptographically matching `garde.lock`.
///
/// # Errors
///
/// Returns [`EngineError`] if `garde-fou.toml` is missing, malformed, or `garde.lock` cannot be written.
pub fn run_lock(root: &Path) -> Result<String, EngineError> {
    let _ = load_workspace_config(root)?;
    write_garde_lock(root)
}

/// Loads `garde-fou.toml` from `root` and executes parallel + workspace + security analysis.
///
/// # Errors
///
/// Returns [`EngineError`] if configuration loading, discovery, I/O, or parsing fails.
pub fn run_check_with_options(
    root: &Path,
    options: &CheckOptions,
) -> Result<CheckReport, EngineError> {
    let loaded = load_workspace_config(root)?;
    let lock_status = verify_garde_lock(root, loaded.analyzer.integrity.require_lock)?;
    let cache = loaded
        .cache_enabled
        .then(|| DiskCache::new(root, &loaded.cache_dir));
    execute_pipeline(
        root,
        options,
        &loaded.analyzer,
        (cache.as_ref(), &lock_status),
    )
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
    let lock_status = if config.integrity.require_lock {
        verify_garde_lock(root, true)?
    } else {
        LockStatus::NotRequired
    };
    execute_pipeline(
        root,
        &CheckOptions::new(scope, true),
        config,
        (None, &lock_status),
    )
}
