//! Zero-dependency Safe Rust metadata-fingerprint workspace watcher (`--watch` & live TUI).
//!
//! Tracks `(len, mtime_ns)` per non-ignored repository file and `garde-fou.toml`,
//! strictly excluding `.git/` and `.garde-fou-cache/` to prevent self-trigger loops.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use gf_core::Config;

use crate::discovery::discover_files;
use crate::types::{DEFAULT_CONFIG_FILENAME, EngineError, ScanScope};

/// Lightweight filesystem metadata stamp for change detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileStamp {
    /// File length in bytes.
    pub len: u64,
    /// Modification timestamp in nanoseconds since `UNIX_EPOCH`.
    pub mtime_ns: u128,
}

/// Deterministic point-in-time snapshot of all watched workspace files.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceSnapshot {
    /// Map of normalized repository-relative paths to their [`FileStamp`].
    pub files: BTreeMap<String, FileStamp>,
    /// Stamp of root `garde-fou.toml` (when present).
    pub manifest_stamp: Option<FileStamp>,
}

/// Change summary produced by [`diff_snapshots`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchDelta {
    /// Created or modified repository-relative paths.
    pub changed: Vec<String>,
    /// Deleted repository-relative paths.
    pub removed: Vec<String>,
    /// `true` when `garde-fou.toml` was created, modified, or removed.
    pub config_changed: bool,
}

impl WatchDelta {
    /// Formats a concise one-line summary (`Δ modifiés : src/a.rs, src/b.rs (+1)`).
    #[must_use]
    pub fn summary_short(&self) -> String {
        let mut names: Vec<&str> = self.changed.iter().map(String::as_str).collect();
        names.extend(self.removed.iter().map(String::as_str));
        if names.is_empty() && self.config_changed {
            return String::from(DEFAULT_CONFIG_FILENAME);
        }
        let shown: Vec<&str> = names.iter().take(3).copied().collect();
        let extra = names.len().saturating_sub(shown.len());
        if extra > 0 {
            format!("{} (+{extra})", shown.join(", "))
        } else {
            shown.join(", ")
        }
    }
}

/// Computes the exact delta between two [`WorkspaceSnapshot`]s, returning `None` when identical.
#[must_use]
pub fn diff_snapshots(old: &WorkspaceSnapshot, new: &WorkspaceSnapshot) -> Option<WatchDelta> {
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for (path, stamp) in &new.files {
        if old.files.get(path) != Some(stamp) {
            changed.push(path.clone());
        }
    }
    for path in old.files.keys() {
        if !new.files.contains_key(path) {
            removed.push(path.clone());
        }
    }
    let config_changed = old.manifest_stamp != new.manifest_stamp;
    if changed.is_empty() && removed.is_empty() && !config_changed {
        return None;
    }
    Some(WatchDelta {
        changed,
        removed,
        config_changed,
    })
}

fn read_stamp(path: &Path) -> Option<FileStamp> {
    let meta = fs::metadata(path).ok()?;
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some(FileStamp {
        len: meta.len(),
        mtime_ns,
    })
}

fn is_ignored_watch_path(rel: &str, cache_dir: &str) -> bool {
    let clean_cache = cache_dir.trim_matches('/');
    rel.starts_with(".git/")
        || rel == ".git"
        || (!clean_cache.is_empty()
            && (rel == clean_cache || rel.starts_with(&format!("{clean_cache}/"))))
}

/// Captures a deterministic [`WorkspaceSnapshot`] of all non-ignored files under `root`.
///
/// # Errors
///
/// Returns [`EngineError`] if workspace file discovery fails.
pub fn capture_snapshot(
    root: &Path,
    config: &Config,
    cache_dir: &str,
) -> Result<WorkspaceSnapshot, EngineError> {
    let discovered = discover_files(root, ScanScope::All, config)?;
    let mut files = BTreeMap::new();
    for repo_path in discovered {
        let rel = repo_path.as_str();
        if is_ignored_watch_path(rel, cache_dir) {
            continue;
        }
        if let Some(stamp) = read_stamp(&root.join(rel)) {
            files.insert(String::from(rel), stamp);
        }
    }
    let manifest_stamp = read_stamp(&root.join(DEFAULT_CONFIG_FILENAME));
    Ok(WorkspaceSnapshot {
        files,
        manifest_stamp,
    })
}
