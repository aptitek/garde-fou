//! Content-addressed `BLAKE3` on-disk cache storing per-file diagnostics.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Diagnostic, RepoPath};
use serde::{Deserialize, Serialize};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Deterministic `BLAKE3` 256-bit digest used for incremental analysis caching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Hashes arbitrary bytes with `BLAKE3`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Computes the composite per-file cache key:
    /// `BLAKE3(engine_version || config_hash || repo_path || content_hash)`.
    #[must_use]
    pub fn for_file(
        engine_version: &str,
        config_hash: Self,
        repo_path: &RepoPath,
        content_hash: Self,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(engine_version.as_bytes());
        hasher.update(&[0]);
        hasher.update(&config_hash.0);
        hasher.update(&[0]);
        hasher.update(repo_path.as_str().as_bytes());
        hasher.update(&[0]);
        hasher.update(&content_hash.0);
        Self(*hasher.finalize().as_bytes())
    }

    /// Encodes the 32-byte digest as lowercase 64-character hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

#[derive(Serialize, Deserialize)]
struct CachedEntry {
    diagnostics: Vec<Diagnostic>,
}

/// Sharded filesystem cache backed by `.garde-fou-cache/v1/`.
#[derive(Clone, Debug)]
pub struct DiskCache {
    base_dir: PathBuf,
}

impl DiskCache {
    /// Creates a [`DiskCache`] handle rooted at `<workspace_root>/<cache_dir>/v1`.
    #[must_use]
    pub fn new(workspace_root: &Path, cache_dir: &str) -> Self {
        Self {
            base_dir: workspace_root.join(cache_dir).join("v1"),
        }
    }

    fn entry_path(&self, key: CacheKey) -> PathBuf {
        let hex = key.to_hex();
        let shard = hex.get(0..2).unwrap_or("00");
        self.base_dir.join(shard).join(format!("{hex}.json"))
    }

    /// Looks up cached diagnostics for `key`, returning `None` on miss or corrupt data.
    #[must_use]
    pub fn get(&self, key: CacheKey) -> Option<Vec<Diagnostic>> {
        let raw = fs::read_to_string(self.entry_path(key)).ok()?;
        let parsed: CachedEntry = serde_json::from_str(&raw).ok()?;
        Some(parsed.diagnostics)
    }

    /// Atomically persists `diagnostics` under `key`.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if creating shard directories or writing the entry fails.
    pub fn put(&self, key: CacheKey, diagnostics: &[Diagnostic]) -> Result<(), std::io::Error> {
        let final_path = self.entry_path(key);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_path = final_path.with_extension(format!("tmp.{pid}.{seq}"));
        let payload = serde_json::to_vec(&CachedEntry {
            diagnostics: diagnostics.to_vec(),
        })
        .map_err(std::io::Error::other)?;
        fs::write(&tmp_path, payload)?;
        fs::rename(&tmp_path, &final_path)
    }
}
