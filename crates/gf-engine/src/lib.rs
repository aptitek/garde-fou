//! Imperative shell orchestrating strict `TOML` config, `uv`-style `init`, context detection,
//! parallel `rayon` analysis, `.garde-fou-cache/v2/` caching, and `SARIF` / `LCOV` gates.

pub mod cache;
pub mod config_loader;
mod config_schema;
pub mod context_detector;
pub mod discovery;
pub mod init;
mod runner;
pub mod scaffold;
pub mod scanners;

use core::fmt;
use std::path::Path;

use gf_core::{Config, Diagnostic, ProjectContext, RepoPathError};
use gf_parsers::ParseError;

pub use cache::{CacheKey, CachedFileArtifact, DiskCache};
pub use config_loader::{LoadedWorkspaceConfig, load_workspace_config};
pub use context_detector::detect_project_context;
pub use discovery::discover_files;
pub use init::{InitReport, InitRequest, run_init};
use runner::execute_pipeline;
pub use scaffold::{DEFAULT_CONFIG_FILENAME, InitOutcome, scaffold_config};
pub use scanners::sarif_ingest::{SarifMapping, ingest_sarif_json};

/// Scope of files selected for a check run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanScope {
    /// Analyze the full repository workspace.
    All,
    /// Analyze staged Git changes while still enforcing workspace-wide rules via `v2` cache.
    StagedOnly,
}

/// Execution flags controlling [`run_check_with_options`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckOptions {
    /// File selection scope (`All` vs `StagedOnly`).
    pub scope: ScanScope,
    /// Bypass cached artifacts and force full `Tree-sitter` re-evaluation.
    pub no_cache: bool,
    /// Upgrade `auto` scanner policies to `required` (`GF043` when a binary is missing).
    pub strict_tools: bool,
    /// Optional explicit path to an `LCOV` coverage report (`GF050`).
    pub lcov_path: Option<String>,
}

impl CheckOptions {
    /// Builds standard [`CheckOptions`] for `scope` (`no_cache` optionally set).
    #[must_use]
    pub const fn new(scope: ScanScope, no_cache: bool) -> Self {
        Self {
            scope,
            no_cache,
            strict_tools: false,
            lcov_path: None,
        }
    }
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self::new(ScanScope::All, false)
    }
}

/// Summary report returned by [`run_check`] and [`run_check_with_options`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckReport {
    /// Requested scan scope.
    pub scope: ScanScope,
    /// Active detected + configured project context (`languages` × `features` × `frameworks`).
    pub context: ProjectContext,
    /// Total number of target source files checked in this scope.
    pub files_checked: usize,
    /// Number of files served directly from `.garde-fou-cache/v2/`.
    pub cache_hits: usize,
    /// Number of files parsed from source on cache miss.
    pub cache_misses: usize,
    /// Hexadecimal `BLAKE3` fingerprint summarizing the analyzed workspace state.
    pub cache_fingerprint: String,
    /// Deterministically sorted local, workspace-global, security, and coverage diagnostics.
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
    /// Strict `garde-fou.toml`, CLI flag, or `SARIF` configuration error.
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
    execute_pipeline(root, &CheckOptions::new(scope, true), config, None)
}
