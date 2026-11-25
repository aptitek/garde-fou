//! Workspace-level SQL dialect inference (`wrangler.toml` / `wrangler.jsonc` `d1_databases`
//! -> `CloudflareD1`, `drizzle.config.*` -> `CloudflareD1` / `Sqlite` / `MySql` / `Postgres`).

use std::fs;
use std::path::Path;

use gf_core::{SqlDialectKind, SqlDialectSetting};

const WRANGLER_CONFIGS: &[&str] = &["wrangler.toml", "wrangler.jsonc", "wrangler.json"];

const DRIZZLE_CONFIGS: &[&str] = &[
    "drizzle.config.ts",
    "drizzle.config.js",
    "drizzle.config.mjs",
    "drizzle.config.json",
];

const DRIZZLE_DIALECT_MARKERS: &[(&[&str], SqlDialectKind)] = &[
    (&["\"d1\"", "'d1'", "d1-http"], SqlDialectKind::CloudflareD1),
    (&["\"sqlite\"", "'sqlite'", "turso"], SqlDialectKind::Sqlite),
    (&["\"mysql\"", "'mysql'"], SqlDialectKind::MySql),
    (
        &["\"postgresql\"", "'postgresql'"],
        SqlDialectKind::Postgres,
    ),
];

fn infer_from_drizzle_text(raw: &str) -> Option<SqlDialectKind> {
    let low = raw.to_ascii_lowercase();
    for (needles, kind) in DRIZZLE_DIALECT_MARKERS {
        if needles.iter().any(|n| low.contains(n)) {
            return Some(*kind);
        }
    }
    None
}

/// Resolves the workspace-wide default [`SqlDialectKind`] hint from `[sql] dialect`,
/// `wrangler.toml`/`wrangler.jsonc`, or `drizzle.config.*`.
#[must_use]
pub fn detect_workspace_sql_dialect(
    root: &Path,
    configured: SqlDialectSetting,
) -> Option<SqlDialectKind> {
    if let SqlDialectSetting::Explicit(kind) = configured {
        return Some(kind);
    }
    for name in WRANGLER_CONFIGS {
        if let Ok(raw) = fs::read_to_string(root.join(name))
            && raw.contains("d1_databases")
        {
            return Some(SqlDialectKind::CloudflareD1);
        }
    }
    for name in DRIZZLE_CONFIGS {
        if let Ok(raw) = fs::read_to_string(root.join(name))
            && let Some(kind) = infer_from_drizzle_text(&raw)
        {
            return Some(kind);
        }
    }
    None
}
