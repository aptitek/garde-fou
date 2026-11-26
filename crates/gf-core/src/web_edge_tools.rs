//! Pure `#![no_std]` Cloudflare Workers/D1/R2, Drizzle ORM, unified [`WebEdgeViolation`],
//! Tier 2 [`WebEdgeConfigIssue`] (`GF081`), Tier 3 [`WebToolsConfig`] (`GF082`), and [`WebEdgeSummary`].

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;
use crate::policies::ScannerPolicy;
pub use crate::web_edge_summary::{WebEdgeSummary, WranglerSummary, is_web_edge_tool_config_file};
use crate::web_edge_violations::{
    A11yViolation, AstroViolation, M3eViolation, PandaViolation, ReactRouterViolation, W3cViolation,
};

/// Cloudflare Workers runtime, `cf`, `D1`, `R2`, and binding coherence violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CloudflareViolation {
    /// Undeclared `env.<BINDING>` access or unused `d1`/`r2` binding in `wrangler.*`.
    BindingCoherence,
    /// Forbidden top-level I/O, entropy, clock, timer, or mutable global `let`/`var` in V8 isolate scope.
    GlobalScopeImpure,
    /// Incompatible Node module (`fs`, `child_process`, `cluster`, `dgram`, `v8`) or `node:*` without `nodejs_compat`.
    ForbiddenNodeApi,
    /// `await env.DB.prepare(...).run()` inside a loop (instead of `env.DB.batch`), dynamic `exec`, or `> 100` `?` params.
    D1NPlusOneOrLimit,
    /// Unsanitized `..` in `R2.get`/`R2.put` key, or mutation of read-only `request.cf`.
    R2StreamAndKeySafety,
}

#[rustfmt::skip]
const CF_META: [(&str, &str); 5] = [
    ("CF-BINDING-COHERENCE", "Declare every accessed `env.<BINDING>` in `wrangler.*` (or `interface Env`) and remove unused `d1_databases`/`r2_buckets` bindings."),
    ("CF-GLOBAL-SCOPE-IMPURE", "Move `fetch`, `crypto.randomUUID`/`getRandomValues`, `Date.now`, timers, and mutable state inside request handlers rather than V8 isolate global scope."),
    ("CF-FORBIDDEN-NODE-API", "Replace unsupported Node modules (`fs`, `child_process`, `v8`) with Web/Workers APIs and enable `\"nodejs_compat\"` in `wrangler.*` when importing `node:*`."),
    ("CF-D1-N-PLUS-ONE-OR-LIMIT", "Use `await env.DB.batch(statements)` instead of awaiting `prepare().run()` in loops, and cap bound `?` parameters at `<= 100` per D1 query."),
    ("CF-R2-STREAM-AND-KEY-SAFETY", "Sanitize R2 object keys against `..` path traversal and never mutate read-only `request.cf` properties."),
];

impl CloudflareViolation {
    /// Canonical `CF-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        CF_META[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        CF_META[self as usize].1
    }
}

/// Drizzle ORM schema & query builder violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DrizzleViolation {
    /// Importing `drizzle-orm/pg-core` in a D1/SQLite project (or `sqlite-core` in a PostgreSQL project).
    DialectMismatch,
    /// `db.update(t).set(...)` or `db.delete(t)` executed without chaining `.where(...)`.
    UnboundedMutation,
    /// `sql.raw(...)` with string interpolation `${...}` or concatenation `+`.
    RawUnsafe,
    /// `.references(() => ...)` foreign-key column without a covering `index(...)` or `uniqueIndex(...)`.
    FkMissingIndex,
    /// Floating-point `real()` / `doublePrecision()` / `float()` column for monetary amounts (`price`, `amount`, `balance`).
    FloatMoney,
}

#[rustfmt::skip]
const DRIZZLE_META: [(&str, &str); 5] = [
    ("DRIZZLE-DIALECT-MISMATCH", "Import table builders from the Drizzle core matching your active database (`drizzle-orm/sqlite-core` for D1/SQLite vs `drizzle-orm/pg-core` for PostgreSQL)."),
    ("DRIZZLE-UNBOUNDED-MUTATION", "Chain `.where(...)` onto every `db.update(table).set(...)` and `db.delete(table)` mutation."),
    ("DRIZZLE-RAW-UNSAFE", "Use parameterized `` sql`... ${param}` `` tagged templates instead of interpolated `sql.raw(...)`."),
    ("DRIZZLE-FK-MISSING-INDEX", "Add an explicit `index('...').on(table.fkCol)` or `uniqueIndex(...)` in the table extra-config callback for every `.references(() => ...)` column."),
    ("DRIZZLE-FLOAT-MONEY", "Store monetary columns (`price`, `amount`, `balance`, `total`, `cost`, `fee`) as integer minor units (`integer`/`bigint`) or exact `numeric` instead of floating-point."),
];

impl DrizzleViolation {
    /// Canonical `DRIZZLE-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        DRIZZLE_META[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        DRIZZLE_META[self as usize].1
    }
}

/// Unified closed sum type across Web, Design System (`Panda`/`M3E`), W3C/WCAG, Cloudflare, and Drizzle rules (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WebEdgeViolation {
    /// Astro `.astro` violation (`ASTRO-*`).
    Astro(AstroViolation),
    /// React Router v7 violation (`RR-*`).
    ReactRouter(ReactRouterViolation),
    /// Panda CSS build-time token violation (`PANDA-*`).
    Panda(PandaViolation),
    /// Material Design 3 Expressive violation (`M3E-*`).
    M3e(M3eViolation),
    /// W3C HTML5 content-model violation (`W3C-*`).
    W3c(W3cViolation),
    /// WAI-ARIA 1.2 & WCAG 2.2 AA violation (`A11Y-*`).
    A11y(A11yViolation),
    /// Cloudflare Workers / D1 / R2 violation (`CF-*`).
    Cloudflare(CloudflareViolation),
    /// Drizzle ORM violation (`DRIZZLE-*`).
    Drizzle(DrizzleViolation),
}

impl WebEdgeViolation {
    /// Canonical rule code (`ASTRO-*`, `RR-*`, `PANDA-*`, `M3E-*`, `W3C-*`, `A11Y-*`, `CF-*`, `DRIZZLE-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Astro(v) => v.code(),
            Self::ReactRouter(v) => v.code(),
            Self::Panda(v) => v.code(),
            Self::M3e(v) => v.code(),
            Self::W3c(v) => v.code(),
            Self::A11y(v) => v.code(),
            Self::Cloudflare(v) => v.code(),
            Self::Drizzle(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Astro(v) => v.remediation(),
            Self::ReactRouter(v) => v.remediation(),
            Self::Panda(v) => v.remediation(),
            Self::M3e(v) => v.remediation(),
            Self::W3c(v) => v.remediation(),
            Self::A11y(v) => v.remediation(),
            Self::Cloudflare(v) => v.remediation(),
            Self::Drizzle(v) => v.remediation(),
        }
    }
}

/// Tier 2 configuration hardening issues for `panda.config.*` and `wrangler.{toml,json,jsonc}` (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WebEdgeConfigIssue {
    /// `panda.config.*` missing `strictTokens: true` or setting `strictTokens: false` (`PANDA-CONFIG-STRICT`).
    PandaStrictTokensMissingOrFalse,
    /// `wrangler.*` missing or invalid `compatibility_date` (`YYYY-MM-DD`, `CF-WRANGLER-CONFIG`).
    WranglerInvalidCompatibilityDate,
    /// `wrangler.*` storing a secret in `[vars]` (`SECRET`, `PASSWORD`, `TOKEN`, `API_KEY`, `PRIVATE_KEY`).
    WranglerPlaintextSecretInVars,
    /// `wrangler.*` leaving `remote = true` enabled on a default production binding.
    WranglerUnguardedRemoteBinding,
    /// `wrangler.*` missing `"nodejs_compat"` in `compatibility_flags` when `node:*` is imported.
    WranglerMissingNodejsCompat,
}

#[rustfmt::skip]
const WEB_EDGE_CFG_META: [(ExternalToolKind, &str, &str); 5] = [
    (ExternalToolKind::Tsc, "strictTokens", "Set `strictTokens: true` (and `strictPropertyValues: true`) inside `defineConfig({ ... })` in `panda.config.*`."),
    (ExternalToolKind::WranglerTypes, "compatibility_date", "Declare a valid `compatibility_date = \"YYYY-MM-DD\"` in `wrangler.toml` / `wrangler.jsonc`."),
    (ExternalToolKind::WranglerTypes, "vars", "Never place credentials in `[vars]`; manage secrets via `wrangler secret put` and `.dev.vars`."),
    (ExternalToolKind::WranglerTypes, "remote", "Remove `remote = true` from default bindings in `wrangler.*` so local dev/tests never mutate production resources."),
    (ExternalToolKind::WranglerTypes, "compatibility_flags", "Add `\"nodejs_compat\"` to `compatibility_flags` in `wrangler.*` when importing `node:*` built-in modules."),
];

impl WebEdgeConfigIssue {
    /// Ecosystem tool associated with this configuration violation.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        WEB_EDGE_CFG_META[self as usize].0
    }

    /// Canonical configuration key path.
    #[must_use]
    pub const fn key(self) -> &'static str {
        WEB_EDGE_CFG_META[self as usize].1
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        WEB_EDGE_CFG_META[self as usize].2
    }
}

/// `[tools.web]` external verifier policy (`astro-check`, `html-validate`, `wrangler-types`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebToolsConfig {
    /// `astro check` policy (`auto` by default).
    pub astro_check: ScannerPolicy,
    /// `html-validate` policy (`off` by default).
    pub html_validate: ScannerPolicy,
    /// `wrangler types` policy (`off` by default).
    pub wrangler_types: ScannerPolicy,
}

impl Default for WebToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

impl WebToolsConfig {
    /// Default `[tools.web]` policy (`astro_check = Auto`, `html_validate = Off`, `wrangler_types = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            astro_check: ScannerPolicy::Auto,
            html_validate: ScannerPolicy::Off,
            wrangler_types: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "web(ac={},hv={},wt={})",
            self.astro_check.as_str(),
            self.html_validate.as_str(),
            self.wrangler_types.as_str()
        )
    }
}
