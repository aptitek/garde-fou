//! Pure `#![no_std]` types for Tier 2 SQL & migration tool configuration hardening (`GF081`)
//! and `[tools.sql]` external verifier policies (`squawk`, `sqruff`, `sqlfluff` — `GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;

/// Specific `.squawk.toml` / `squawk.toml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SquawkConfigIssue {
    /// `excluded_rules` silencing critical zero-downtime migration rules.
    CriticalRuleExcluded,
    /// `excluded_paths` blanket-ignoring `migrations/**` or `**/*.sql`.
    BlanketExcludedPaths,
}

impl SquawkConfigIssue {
    /// Canonical key inside `.squawk.toml`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CriticalRuleExcluded => "excluded_rules",
            Self::BlanketExcludedPaths => "excluded_paths",
        }
    }
}

/// Specific `.sqruff` or `.sqlfluff` INI configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SqruffSqlfluffConfigIssue {
    /// `exclude_rules = all` disabling every SQL rule.
    ExcludeAllRules,
    /// `exclude_rules` silencing critical correctness rules (`AL09`, `CV05`, `L049`, `AM04`).
    CriticalRuleExcluded,
}

impl SqruffSqlfluffConfigIssue {
    /// Canonical key inside `.sqruff` / `.sqlfluff`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::ExcludeAllRules | Self::CriticalRuleExcluded => "sqlfluff.exclude_rules",
        }
    }
}

/// Specific `drizzle.config.*` security weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DrizzleConfigIssue {
    /// Hardcoded database connection URL with inline password outside `process.env.*`.
    HardcodedDatabaseCredentials,
}

impl DrizzleConfigIssue {
    /// Canonical key inside `drizzle.config.*`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::HardcodedDatabaseCredentials => "dbCredentials.url",
        }
    }
}

/// Unified Tier 2 SQL & migration configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SqlToolConfigIssue {
    /// `.squawk.toml` / `squawk.toml` weakening.
    Squawk(SquawkConfigIssue),
    /// `.sqruff` INI weakening.
    Sqruff(SqruffSqlfluffConfigIssue),
    /// `.sqlfluff` INI weakening.
    Sqlfluff(SqruffSqlfluffConfigIssue),
    /// `drizzle.config.*` hardcoded credentials weakening.
    Drizzle(DrizzleConfigIssue),
}

impl SqlToolConfigIssue {
    /// Ecosystem tool affected by this configuration issue.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::Squawk(_) => ExternalToolKind::Squawk,
            Self::Sqruff(_) | Self::Drizzle(_) => ExternalToolKind::Sqruff,
            Self::Sqlfluff(_) => ExternalToolKind::Sqlfluff,
        }
    }

    /// Canonical key inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Squawk(k) => k.key(),
            Self::Sqruff(k) | Self::Sqlfluff(k) => k.key(),
            Self::Drizzle(k) => k.key(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Squawk(_) => {
                "Keep `require-concurrent-index-creation`, `adding-not-null-field`, and `constraint-missing-not-valid` active in `.squawk.toml`."
            }
            Self::Sqruff(_) | Self::Sqlfluff(_) => {
                "Never set `exclude_rules = all` or disable critical SQL semantics rules (`CV05`/`L049`, `AM04`, `AL09`) in `.sqruff` / `.sqlfluff`."
            }
            Self::Drizzle(_) => {
                "Read `dbCredentials.url` from `process.env.DATABASE_URL` instead of hardcoding database credentials in `drizzle.config.*`."
            }
        }
    }
}

/// `[tools.sql]` execution policies for external SQL & migration verifiers (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlToolsConfig {
    /// Policy for `squawk --reporter Json` (`auto` by default, Rust-native).
    pub squawk: ScannerPolicy,
    /// Policy for `sqruff lint -f json` (`auto` by default, Rust-native).
    pub sqruff: ScannerPolicy,
    /// Policy for `sqlfluff lint --format json` (`off` by default due to Python cold-start).
    pub sqlfluff: ScannerPolicy,
}

impl SqlToolsConfig {
    /// Default SQL tool policies (`squawk = Auto`, `sqruff = Auto`, `sqlfluff = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            squawk: ScannerPolicy::Auto,
            sqruff: ScannerPolicy::Auto,
            sqlfluff: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "sql(sqk={},sqr={},flf={})",
            self.squawk.as_str(),
            self.sqruff.as_str(),
            self.sqlfluff.as_str()
        )
    }
}

impl Default for SqlToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` if `path` is a SQL/migration linter or ORM configuration file audited by Tier 2 (`GF081`).
#[must_use]
pub fn is_sql_tool_config_file(path: &RepoPath) -> bool {
    matches!(
        path.file_name(),
        ".squawk.toml"
            | "squawk.toml"
            | ".sqruff"
            | ".sqlfluff"
            | "drizzle.config.ts"
            | "drizzle.config.js"
            | "drizzle.config.mjs"
    )
}
