//! Imperative shell orchestrating filesystem I/O, `BLAKE3` cache keys, and pure analysis.
//!
//! `gf-engine` may read/write files and inspect paths, but never writes directly to
//! `stdout` or `stderr` (reserved exclusively for `gf-cli`).

pub mod scaffold;

use core::fmt;
use std::fs;
use std::path::Path;

use gf_core::{Config, Diagnostic, RepoPath, RepoPathError};
use gf_parsers::{ParseError, parse_source_skeleton};
use gf_rules::evaluate_document;

pub use scaffold::{DEFAULT_CONFIG_FILENAME, InitOutcome, scaffold_config};

/// Deterministic `BLAKE3` content digest used for incremental analysis caching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Hashes arbitrary bytes with `BLAKE3`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Encodes the 32-byte digest as lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

/// Scope of files selected for a check run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanScope {
    /// Analyze the full repository workspace.
    All,
    /// Analyze only staged Git changes (`--staged`).
    StagedOnly,
}

/// Summary report returned by [`run_check`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckReport {
    /// Requested scan scope.
    pub scope: ScanScope,
    /// Number of files analyzed during this run.
    pub files_checked: usize,
    /// Hexadecimal `BLAKE3` fingerprint of the analyzed input state.
    pub cache_fingerprint: String,
    /// Diagnostics produced by `gf-rules` and external runners.
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
    /// Source-to-UAST bridge failure.
    Parse(ParseError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidPath(err) => write!(f, "invalid repository path: {err}"),
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

/// Executes a static analysis pass rooted at `root`.
///
/// # Errors
///
/// Returns [`EngineError`] if reading configuration or source files fails.
pub fn run_check(
    root: &Path,
    scope: ScanScope,
    config: &Config,
) -> Result<CheckReport, EngineError> {
    let manifest_path = root.join(DEFAULT_CONFIG_FILENAME);
    if !manifest_path.is_file() {
        return Ok(CheckReport {
            scope,
            files_checked: 0,
            cache_fingerprint: CacheKey::from_bytes(b"").to_hex(),
            diagnostics: Vec::new(),
        });
    }

    let source = fs::read_to_string(&manifest_path)?;
    let cache_fingerprint = CacheKey::from_bytes(source.as_bytes()).to_hex();
    let repo_path = RepoPath::new(DEFAULT_CONFIG_FILENAME)?;
    let uast = parse_source_skeleton(repo_path, &source)?;
    let diagnostics = evaluate_document(&uast, config);

    Ok(CheckReport {
        scope,
        files_checked: 1,
        cache_fingerprint,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use gf_core::{Config, LimitsConfig, RepoPathError};
    use gf_parsers::ParseError;

    use super::{
        CacheKey, DEFAULT_CONFIG_FILENAME, EngineError, InitOutcome, ScanScope, run_check,
        scaffold_config,
    };

    #[test]
    fn scaffolds_and_checks_workspace_with_blake3_fingerprint()
    -> Result<(), Box<dyn std::error::Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("gf-engine-test-{nanos}"));
        fs::create_dir_all(&dir)?;

        let empty_report = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
        assert_eq!(empty_report.files_checked, 0);
        assert!(empty_report.is_clean());

        let first = scaffold_config(&dir)?;
        assert!(matches!(first, InitOutcome::Created(_)));
        let second = scaffold_config(&dir)?;
        assert!(matches!(second, InitOutcome::AlreadyPresent(_)));

        let clean_report = run_check(&dir, ScanScope::StagedOnly, &Config::strict_defaults())?;
        assert_eq!(clean_report.files_checked, 1);
        assert_eq!(clean_report.cache_fingerprint.len(), 64);
        assert!(clean_report.is_clean());

        let tiny_cfg = Config {
            limits: LimitsConfig {
                max_file_lines: 2,
                ..LimitsConfig::strict_defaults()
            },
            forbid_panicking_constructs: true,
        };
        let failing_report = run_check(&dir, ScanScope::All, &tiny_cfg)?;
        assert!(!failing_report.is_clean());

        let _ = fs::remove_file(dir.join(DEFAULT_CONFIG_FILENAME));
        let _ = fs::remove_dir_all(&dir);

        assert_ne!(EngineError::from(RepoPathError::Empty).to_string(), "");
        assert_ne!(
            EngineError::from(ParseError::InvalidSpan(gf_core::SpanError::ZeroLine)).to_string(),
            ""
        );
        assert_eq!(CacheKey::from_bytes(b"abc").to_hex().len(), 64);
        Ok(())
    }
}
