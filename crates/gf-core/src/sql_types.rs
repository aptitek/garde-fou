//! Pure `#![no_std]` leaf SQL dialect, configuration, migration role, and document summary types.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Concrete SQL dialect used to parse and evaluate a `.sql` document.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SqlDialectKind {
    /// `PostgreSQL` (`$$...$$`, `CONCURRENTLY`, `TIMESTAMPTZ`, `NOT VALID`).
    #[default]
    Postgres,
    /// Standard `SQLite` (`STRICT`, `WITHOUT ROWID`, `CREATE TRIGGER ... BEGIN ... END;`).
    Sqlite,
    /// Cloudflare `D1` managed `SQLite` (forbids explicit `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`).
    CloudflareD1,
    /// `MySQL` / `MariaDB`.
    MySql,
    /// `DuckDB` analytical SQL.
    DuckDb,
    /// ANSI / generic SQL fallback.
    Generic,
}

impl SqlDialectKind {
    /// Parses a case-insensitive SQL dialect name (`postgres`, `sqlite`, `d1`, `mysql`, `duckdb`, `generic`).
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        for (alias, kind) in [
            ("postgres", Self::Postgres),
            ("postgresql", Self::Postgres),
            ("pg", Self::Postgres),
            ("sqlite", Self::Sqlite),
            ("sqlite3", Self::Sqlite),
            ("d1", Self::CloudflareD1),
            ("cloudflare-d1", Self::CloudflareD1),
            ("mysql", Self::MySql),
            ("mariadb", Self::MySql),
            ("duckdb", Self::DuckDb),
            ("generic", Self::Generic),
            ("ansi", Self::Generic),
        ] {
            if trimmed.eq_ignore_ascii_case(alias) {
                return Some(kind);
            }
        }
        None
    }

    /// Canonical lowercase dialect identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Sqlite => "sqlite",
            Self::CloudflareD1 => "d1",
            Self::MySql => "mysql",
            Self::DuckDb => "duckdb",
            Self::Generic => "generic",
        }
    }
}

impl fmt::Display for SqlDialectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configured `[sql] dialect` policy in `garde-fou.toml` (`"auto"` by default).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum SqlDialectSetting {
    /// Automatically infer dialect from `-- garde-dialect:`, `wrangler.toml`, `drizzle.config.*`, or path.
    #[default]
    Auto,
    /// Force a concrete [`SqlDialectKind`] unless overridden per file by `-- garde-dialect:`.
    Explicit(SqlDialectKind),
}

impl SqlDialectSetting {
    /// Parses `"auto"` or any [`SqlDialectKind`] alias.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        if raw.trim().eq_ignore_ascii_case("auto") {
            return Some(Self::Auto);
        }
        SqlDialectKind::from_config_str(raw).map(Self::Explicit)
    }

    /// Canonical lowercase identifier for cache hashing.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Explicit(d) => d.as_str(),
        }
    }
}

/// `[sql]` analyzer policy in `garde-fou.toml`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlConfig {
    /// Default dialect resolution mode (`auto` by default).
    #[serde(default)]
    pub dialect: SqlDialectSetting,
    /// Whether destructive DDL (`DROP TABLE`, `TRUNCATE`, `DROP COLUMN`, `RENAME`) is permitted in up-migrations.
    #[serde(default)]
    pub allow_destructive_migrations: bool,
}

impl SqlConfig {
    /// Strict zero-downtime defaults (`dialect = Auto`, `allow_destructive_migrations = false`).
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            dialect: SqlDialectSetting::Auto,
            allow_destructive_migrations: false,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "sql(d={},destr={})",
            self.dialect.as_str(),
            u8::from(self.allow_destructive_migrations)
        )
    }
}

/// Semantic role of a `.sql` file within the repository.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SqlRole {
    /// Ordered schema migration (`migrations/**`, `drizzle/**`, `prisma/migrations/**`, `db/migrate/**`).
    Migration {
        /// Optional normalized numeric version prefix (e.g. `"0004"` or `"202601011200"`).
        version_prefix: Option<String>,
        /// Stem without `.up.sql` / `.down.sql` / `.sql` suffix for orphan `.down.sql` pairing.
        migration_stem: String,
        /// `true` when the file is a rollback migration (`*.down.sql` or `down.sql`).
        is_down: bool,
    },
    /// Standalone SQL query, view definition, or declarative schema file.
    QueryOrSchema,
}

/// Destructive DDL sighting recorded in an up-migration, gated by `SqlConfig::allow_destructive_migrations`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DestructiveDdlFinding {
    /// 1-based span of the destructive statement.
    pub span: Span,
    /// Description of the destructive operation (`DROP TABLE`, `TRUNCATE`, `DROP COLUMN`, `RENAME`).
    pub detail: String,
}

/// Lightweight SQL document summary stored on [`crate::UastDocument`] for workspace migration graph checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SqlDocument {
    /// Resolved SQL dialect used to parse this document.
    pub dialect: SqlDialectKind,
    /// Classified role (`Migration` vs `QueryOrSchema`).
    pub role: SqlRole,
    /// Parent directory grouping co-sequenced migrations (e.g. `"migrations"` or `"prisma/migrations"`).
    pub migration_group_dir: String,
    /// Destructive DDL operations found on existing tables in up-migrations.
    pub destructive_ddl: Vec<DestructiveDdlFinding>,
}
