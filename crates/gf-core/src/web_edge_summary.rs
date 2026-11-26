//! Pure `#![no_std]` cross-file Wrangler, Cloudflare, Drizzle, and `Forme` motion summaries
//! attached to [`crate::UastDocument`].

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
use crate::path::RepoPath;
use crate::sql_types::SqlDialectKind;

/// Returns `true` if `path` is a `panda.config.*` or `wrangler.{toml,json,jsonc}` config file.
#[must_use]
pub fn is_web_edge_tool_config_file(path: &RepoPath) -> bool {
    matches!(
        path.file_name(),
        "panda.config.ts"
            | "panda.config.mjs"
            | "panda.config.js"
            | "wrangler.toml"
            | "wrangler.json"
            | "wrangler.jsonc"
    )
}

/// Extracted `wrangler.{toml,json,jsonc}` binding and compatibility metadata for workspace coherence (`CF-BINDING-COHERENCE`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WranglerSummary {
    /// `true` if `compatibility_flags` includes `"nodejs_compat"` or `"nodejs_compat_v2"`.
    pub has_nodejs_compat: bool,
    /// `true` if `d1_databases` is non-empty.
    pub has_d1: bool,
    /// All binding names declared across `d1_databases`, `r2_buckets`, `kv_namespaces`, `queues`, `ai`, `durable_objects`, `services`, and `vars`.
    pub declared_bindings: BTreeSet<String>,
    /// Specifically `d1_databases` and `r2_buckets` `(binding_name, span)` pairs checked for dead bindings.
    pub d1_r2_bindings: Vec<(String, Span)>,
    /// Span of the root configuration.
    pub span: Span,
}

/// Cross-file Web, Cloudflare, Drizzle, and `Forme` motion metadata attached to [`crate::UastDocument`].
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebEdgeSummary {
    /// Parsed `wrangler.*` summary when this document is a Wrangler configuration file.
    pub wrangler: Option<WranglerSummary>,
    /// `(binding_name, span)` for every `env.<BINDING>` access in `Code`.
    pub env_bindings_accessed: Vec<(String, Span)>,
    /// Environment binding names declared in `interface Env` / `type Env` or `.dev.vars.example`.
    pub documented_env_keys: BTreeSet<String>,
    /// `(module_specifier, span)` for `node:*` imports in `Code`.
    pub node_colon_imports: Vec<(String, Span)>,
    /// `(dialect_kind, span)` for `drizzle-orm/{sqlite,pg,mysql}-core` imports in `Code`.
    pub drizzle_dialect_imports: Vec<(SqlDialectKind, Span)>,
    /// Explicit Drizzle dialect configured in `drizzle.config.*` (`sqlite`, `postgresql`, `mysql`).
    pub configured_drizzle_dialect: Option<SqlDialectKind>,
    /// Spans of `@keyframes` or spatial `animation:`/`transition:` in a `Forme` CSS stylesheet.
    pub forme_spatial_motion_spans: Vec<Span>,
    /// `true` if this `Forme` stylesheet declares `@media (prefers-reduced-motion: reduce)`.
    pub forme_has_reduced_motion: bool,
}
