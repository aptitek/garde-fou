//! Content-addressed `BLAKE3` `.garde-fou-cache/v10/` storing `UastDocument` + diagnostics.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Diagnostic, RepoPath, UastDocument};
use serde::{Deserialize, Serialize};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

/// Deterministic `BLAKE3` 256-bit digest used for incremental analysis caching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Hashes arbitrary in-memory bytes with `BLAKE3`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Streams `path` in `64 KiB` chunks into `BLAKE3` without loading the full file in RAM.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if opening or reading `path` fails.
    pub fn from_file_stream(path: &Path) -> Result<Self, std::io::Error> {
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0_u8; STREAM_CHUNK_BYTES];
        loop {
            let read_bytes = file.read(&mut buf)?;
            if read_bytes == 0 {
                break;
            }
            if let Some(chunk) = buf.get(..read_bytes) {
                hasher.update(chunk);
            }
        }
        Ok(Self(*hasher.finalize().as_bytes()))
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

/// Cached per-file analysis artifact persisted in `.garde-fou-cache/v9/`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CachedFileArtifact {
    /// Normalized `UAST` document enabling sub-second cross-file global evaluation.
    pub document: UastDocument,
    /// Single-file diagnostics produced by `evaluate_document`.
    pub local_diagnostics: Vec<Diagnostic>,
}

/// Sharded filesystem cache backed by `.garde-fou-cache/v9/`.
#[derive(Clone, Debug)]
pub struct DiskCache {
    base_dir: PathBuf,
}

impl DiskCache {
    /// Creates a [`DiskCache`] handle rooted at `<workspace_root>/<cache_dir>/v9`.
    #[must_use]
    pub fn new(workspace_root: &Path, cache_dir: &str) -> Self {
        Self {
            base_dir: workspace_root.join(cache_dir).join("v11"),
        }
    }

    fn entry_path(&self, key: CacheKey) -> PathBuf {
        let hex = key.to_hex();
        let shard = hex.get(0..2).unwrap_or("00");
        self.base_dir.join(shard).join(format!("{hex}.json"))
    }

    /// Looks up a cached [`CachedFileArtifact`] for `key`, returning `None` on miss.
    #[must_use]
    pub fn get(&self, key: CacheKey) -> Option<CachedFileArtifact> {
        let raw = fs::read_to_string(self.entry_path(key)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Atomically persists `artifact` under `key`.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if creating shard directories or writing the entry fails.
    pub fn put(&self, key: CacheKey, artifact: &CachedFileArtifact) -> Result<(), std::io::Error> {
        let final_path = self.entry_path(key);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_path = final_path.with_extension(format!("tmp.{pid}.{seq}"));
        let payload = serde_json::to_vec(artifact).map_err(std::io::Error::other)?;
        fs::write(&tmp_path, payload)?;
        fs::rename(&tmp_path, &final_path)
    }
}
