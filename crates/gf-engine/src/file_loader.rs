//! Incremental `v6` file-level analysis and stream-hashed binary asset loader.

use std::borrow::Cow;
use std::fs;
use std::path::Path;

use gf_core::{Config, Language, RepoPath, SqlDialectKind};
use gf_parsers::{build_binary_asset_document, detect_language, parse_source_with_context};
use gf_rules::evaluate_document;

use crate::cache::{CacheKey, CachedFileArtifact, DiskCache};
use crate::types::EngineError;

/// Per-file analysis context passed to [`load_or_analyze_file`].
pub struct FileScanCtx<'a> {
    pub config: &'a Config,
    pub config_hash: CacheKey,
    pub disk_cache: Option<&'a DiskCache>,
    pub bypass_read: bool,
    pub sql_dialect_hint: Option<SqlDialectKind>,
}

/// Analyzed or `v6`-cached single-file outcome.
pub struct FileOutcome {
    pub file_key: CacheKey,
    pub was_hit: bool,
    pub artifact: CachedFileArtifact,
}

fn load_binary_asset_outcome(
    root: &Path,
    repo_path: &RepoPath,
    ctx: &FileScanCtx<'_>,
) -> Result<FileOutcome, EngineError> {
    let full_path = root.join(repo_path.as_str());
    let byte_size = fs::metadata(&full_path)?.len();
    let content_hash = CacheKey::from_file_stream(&full_path)?;
    let file_key = CacheKey::for_file(
        env!("CARGO_PKG_VERSION"),
        ctx.config_hash,
        repo_path,
        content_hash,
    );
    if !ctx.bypass_read
        && let Some(cache) = ctx.disk_cache
        && let Some(artifact) = cache.get(file_key)
    {
        return Ok(FileOutcome {
            file_key,
            was_hit: true,
            artifact,
        });
    }
    let document = build_binary_asset_document(repo_path, byte_size)?;
    let local_diagnostics = evaluate_document(&document, ctx.config);
    let artifact = CachedFileArtifact {
        document,
        local_diagnostics,
    };
    if let Some(cache) = ctx.disk_cache {
        cache.put(file_key, &artifact)?;
    }
    Ok(FileOutcome {
        file_key,
        was_hit: false,
        artifact,
    })
}

fn normalize_lf(raw_source: &str) -> Cow<'_, str> {
    if raw_source.contains('\r') {
        Cow::Owned(raw_source.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(raw_source)
    }
}

/// Loads `repo_path` from the `v12` cache or parses + evaluates it, persisting to disk on miss.
pub fn load_or_analyze_file(
    root: &Path,
    repo_path: &RepoPath,
    ctx: &FileScanCtx<'_>,
) -> Result<FileOutcome, EngineError> {
    if detect_language(repo_path) == Language::BinaryAsset {
        return load_binary_asset_outcome(root, repo_path, ctx);
    }
    let raw_source = fs::read_to_string(root.join(repo_path.as_str()))?;
    let source = normalize_lf(&raw_source);
    let content_hash = CacheKey::from_bytes(source.as_bytes());
    let file_key = CacheKey::for_file(
        env!("CARGO_PKG_VERSION"),
        ctx.config_hash,
        repo_path,
        content_hash,
    );
    if !ctx.bypass_read
        && let Some(cache) = ctx.disk_cache
        && let Some(artifact) = cache.get(file_key)
    {
        return Ok(FileOutcome {
            file_key,
            was_hit: true,
            artifact,
        });
    }
    let document = parse_source_with_context(
        repo_path,
        &source,
        &ctx.config.domains,
        ctx.sql_dialect_hint,
    )?;
    let local_diagnostics = evaluate_document(&document, ctx.config);
    let artifact = CachedFileArtifact {
        document,
        local_diagnostics,
    };
    if let Some(cache) = ctx.disk_cache {
        cache.put(file_key, &artifact)?;
    }
    Ok(FileOutcome {
        file_key,
        was_hit: false,
        artifact,
    })
}
