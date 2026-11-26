//! Cache inspection and lifecycle management (`garde cache status|prune|clean`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::cache::{ACTIVE_CACHE_SCHEMA, CachedFileArtifact};
use crate::config_loader::load_workspace_config;
use crate::types::EngineError;

/// Diagnostic metrics for `.garde-fou-cache/` returned by [`cache_status`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheReport {
    /// Absolute path to the workspace cache root (`.garde-fou-cache`).
    pub cache_root: PathBuf,
    /// Current schema version folder name (e.g. `"v16"`).
    pub active_schema: String,
    /// Number of valid cached file entries in the active schema.
    pub active_entries: usize,
    /// Total bytes occupied by valid entries in the active schema.
    pub active_bytes: u64,
    /// Outdated schema subdirectories (`v1..v{N-1}`) detected on disk.
    pub stale_schemas: Vec<String>,
    /// Total file entries inside outdated schema folders.
    pub stale_entries: usize,
    /// Total bytes occupied by outdated schema folders.
    pub stale_bytes: u64,
    /// Active-schema entries whose source file no longer exists on disk.
    pub orphan_entries: usize,
    /// Total bytes occupied by orphaned entries in the active schema.
    pub orphan_bytes: u64,
}

impl CacheReport {
    /// Total bytes across active, stale, and orphaned cache entries.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.active_bytes
            .saturating_add(self.stale_bytes)
            .saturating_add(self.orphan_bytes)
    }

    /// Total number of cached files across all schema directories.
    #[must_use]
    pub const fn total_entries(&self) -> usize {
        self.active_entries
            .saturating_add(self.stale_entries)
            .saturating_add(self.orphan_entries)
    }
}

/// Summary returned by [`cache_prune`] and [`cache_clean`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheCleanSummary {
    /// Number of cache entry files removed.
    pub removed_entries: usize,
    /// Number of obsolete or root directories removed.
    pub removed_dirs: usize,
    /// Total bytes reclaimed on disk.
    pub freed_bytes: u64,
}

fn dir_stats_recursive(dir: &Path) -> (usize, u64) {
    let Ok(rd) = fs::read_dir(dir) else {
        return (0, 0);
    };
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    for item in rd.flatten() {
        let p = item.path();
        if p.is_dir() {
            let (sub_e, sub_b) = dir_stats_recursive(&p);
            entries += sub_e;
            bytes = bytes.saturating_add(sub_b);
        } else if let Ok(meta) = item.metadata() {
            entries += 1;
            bytes = bytes.saturating_add(meta.len());
        }
    }
    (entries, bytes)
}

fn is_active_entry_orphan(workspace_root: &Path, entry_path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(entry_path) else {
        return true;
    };
    let Ok(artifact) = serde_json::from_str::<CachedFileArtifact>(&raw) else {
        return true;
    };
    !workspace_root
        .join(artifact.document.path.as_str())
        .is_file()
}

fn scan_shard_dir(
    workspace_root: &Path,
    shard_path: &Path,
    report: &mut CacheReport,
    orphans: &mut Vec<(PathBuf, u64)>,
) {
    let Ok(files) = fs::read_dir(shard_path) else {
        return;
    };
    for file in files.flatten() {
        let p = file.path();
        if !p.is_file() {
            continue;
        }
        let len = file.metadata().map_or(0, |m| m.len());
        if is_active_entry_orphan(workspace_root, &p) {
            report.orphan_entries += 1;
            report.orphan_bytes = report.orphan_bytes.saturating_add(len);
            orphans.push((p, len));
        } else {
            report.active_entries += 1;
            report.active_bytes = report.active_bytes.saturating_add(len);
        }
    }
}

fn scan_active_schema(
    workspace_root: &Path,
    active_dir: &Path,
    report: &mut CacheReport,
    orphans: &mut Vec<(PathBuf, u64)>,
) {
    let Ok(shards) = fs::read_dir(active_dir) else {
        return;
    };
    for shard in shards.flatten() {
        let shard_path = shard.path();
        if shard_path.is_dir() {
            scan_shard_dir(workspace_root, &shard_path, report, orphans);
        }
    }
}

fn inspect_cache(root: &Path) -> Result<(CacheReport, Vec<(PathBuf, u64)>), EngineError> {
    let loaded = load_workspace_config(root)?;
    let cache_root = root.join(&loaded.cache_dir);
    let mut report = CacheReport {
        cache_root: cache_root.clone(),
        active_schema: String::from(ACTIVE_CACHE_SCHEMA),
        ..CacheReport::default()
    };
    let mut orphans = Vec::new();
    if !cache_root.is_dir() {
        return Ok((report, orphans));
    }

    let mut children: Vec<PathBuf> = fs::read_dir(&cache_root)?
        .flatten()
        .map(|e| e.path())
        .collect();
    children.sort();

    for child in children {
        if !child.is_dir() {
            continue;
        }
        let name = child
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == ACTIVE_CACHE_SCHEMA {
            scan_active_schema(root, &child, &mut report, &mut orphans);
        } else {
            let (cnt, bytes) = dir_stats_recursive(&child);
            report.stale_schemas.push(String::from(name));
            report.stale_entries += cnt;
            report.stale_bytes = report.stale_bytes.saturating_add(bytes);
        }
    }
    Ok((report, orphans))
}

/// Inspects `.garde-fou-cache/` in `root` and returns a [`CacheReport`].
///
/// # Errors
///
/// Returns [`EngineError`] if workspace config loading or directory traversal fails.
pub fn cache_status(root: &Path) -> Result<CacheReport, EngineError> {
    let (report, _) = inspect_cache(root)?;
    Ok(report)
}

/// Prunes outdated schema directories (`v1..v{N-1}`) and orphaned entries while keeping
/// the active warm cache intact.
///
/// # Errors
///
/// Returns [`EngineError`] if reading or removing stale cache artifacts fails.
pub fn cache_prune(root: &Path) -> Result<CacheCleanSummary, EngineError> {
    let (report, orphans) = inspect_cache(root)?;
    let mut summary = CacheCleanSummary::default();

    for stale_name in &report.stale_schemas {
        let dir = report.cache_root.join(stale_name);
        if dir.is_dir() {
            let (cnt, bytes) = dir_stats_recursive(&dir);
            fs::remove_dir_all(&dir)?;
            summary.removed_entries += cnt;
            summary.removed_dirs += 1;
            summary.freed_bytes = summary.freed_bytes.saturating_add(bytes);
        }
    }
    for (orphan_path, len) in orphans {
        if orphan_path.is_file() {
            fs::remove_file(&orphan_path)?;
            summary.removed_entries += 1;
            summary.freed_bytes = summary.freed_bytes.saturating_add(len);
        }
    }
    Ok(summary)
}

/// Removes the entire `.garde-fou-cache/` directory and reports bytes freed.
///
/// # Errors
///
/// Returns [`EngineError`] if workspace config loading or directory removal fails.
pub fn cache_clean(root: &Path) -> Result<CacheCleanSummary, EngineError> {
    let report = cache_status(root)?;
    if !report.cache_root.is_dir() {
        return Ok(CacheCleanSummary::default());
    }
    let (cnt, bytes) = dir_stats_recursive(&report.cache_root);
    fs::remove_dir_all(&report.cache_root)?;
    Ok(CacheCleanSummary {
        removed_entries: cnt,
        removed_dirs: 1,
        freed_bytes: bytes,
    })
}
