//! Pure `#![no_std]` leaf sum types for universal SQL semantics, embedded SQL injection,
//! and zero-downtime database migration rules (`GF080`).

use serde::{Deserialize, Serialize};

pub use crate::sql_types::{
    DestructiveDdlFinding, SqlConfig, SqlDialectKind, SqlDialectSetting, SqlDocument, SqlRole,
};

/// Universal SQL query & embedded SQL injection violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SqlViolation {
    /// `= NULL`, `!= NULL`, or `<> NULL` yielding `UNKNOWN` in SQL three-valued logic.
    NullComparison,
    /// `NOT IN (SELECT ...)` returning zero rows whenever the subquery yields a single `NULL`.
    NotInSubquery,
    /// `UPDATE` or `DELETE` without `WHERE` or with a trivial tautology (`WHERE 1 = 1`, `WHERE true`).
    UnboundedMutation,
    /// `SELECT *` / `SELECT t.*` in `CREATE [MATERIALIZED] VIEW` or `QueryOrSchema` queries.
    SelectStar,
    /// String interpolation (`${...}`, `+`, `f"..."`, `.format()`, `%`, `format!`) passed to SQL execution.
    InjectionInterpolation,
}

impl SqlViolation {
    /// Canonical `SQL-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NullComparison => "SQL-NULL-COMPARISON",
            Self::NotInSubquery => "SQL-NOT-IN-SUBQUERY",
            Self::UnboundedMutation => "SQL-UNBOUNDED-MUTATION",
            Self::SelectStar => "SQL-SELECT-STAR",
            Self::InjectionInterpolation => "SQL-INJECTION-INTERPOLATION",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::NullComparison => {
                "Replace `= NULL` / `!= NULL` / `<> NULL` with `IS NULL`, `IS NOT NULL`, or `IS [NOT] DISTINCT FROM NULL`."
            }
            Self::NotInSubquery => {
                "Rewrite `NOT IN (SELECT ...)` as `NOT EXISTS (SELECT 1 FROM ... WHERE ...)` so nullable columns never empty the result set."
            }
            Self::UnboundedMutation => {
                "Add a selective, non-tautological `WHERE` predicate to every `UPDATE` and `DELETE` statement."
            }
            Self::SelectStar => {
                "Project explicit column names instead of `SELECT *` so schema evolutions never break column ordering or leak new fields."
            }
            Self::InjectionInterpolation => {
                "Never interpolate variables into SQL strings; use parameterized placeholders (`$1`, `?`, `%s` tuple args, `sqlx::query(...).bind(...)`, or tagged `` sql`...` ``)."
            }
        }
    }
}

/// Zero-downtime database migration & cross-file migration graph violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MigrationViolation {
    /// `PostgreSQL` `CREATE [UNIQUE] INDEX` or `DROP INDEX` without `CONCURRENTLY` on an existing table.
    PgConcurrentIndex,
    /// `PostgreSQL` `CREATE/DROP INDEX CONCURRENTLY` or `REINDEX CONCURRENTLY` inside an explicit transaction.
    PgConcurrentlyInTx,
    /// `PostgreSQL` `ADD COLUMN ... NOT NULL` without safe non-volatile default, or unvalidated `SET NOT NULL`.
    PgAddNotNullUnsafe,
    /// `PostgreSQL` `ADD CONSTRAINT` (`FOREIGN KEY`/`CHECK` without `NOT VALID`, `UNIQUE` without `USING INDEX`, or `VALIDATE` in same tx).
    PgConstraintNotValid,
    /// `PostgreSQL` `ALTER COLUMN ... TYPE ...` triggering a full table rewrite under `ACCESS EXCLUSIVE`.
    PgAlterColumnType,
    /// `PostgreSQL` discouraged `CHAR(n)` / `BPCHAR` or `TIMESTAMP [WITHOUT TIME ZONE]` column type.
    PgBanTypes,
    /// Explicit `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT` rejected by Cloudflare `D1` migrations.
    D1ForbidTransaction,
    /// `SQLite` / `D1` `INTEGER PRIMARY KEY AUTOINCREMENT` (`sqlite_sequence` overhead).
    SqliteAutoincrementTrap,
    /// Destructive DDL (`DROP TABLE`, `TRUNCATE`, `DROP COLUMN`, `RENAME`) in an up-migration.
    DestructiveDdl,
    /// Duplicate numeric migration prefix or orphan `.down.sql` without matching up-migration.
    VersionCollisionOrGap,
}

impl MigrationViolation {
    /// Canonical `MIG-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PgConcurrentIndex => "MIG-PG-CONCURRENT-INDEX",
            Self::PgConcurrentlyInTx => "MIG-PG-CONCURRENTLY-IN-TX",
            Self::PgAddNotNullUnsafe => "MIG-PG-ADD-NOT-NULL-UNSAFE",
            Self::PgConstraintNotValid => "MIG-PG-CONSTRAINT-NOT-VALID",
            Self::PgAlterColumnType => "MIG-PG-ALTER-COLUMN-TYPE",
            Self::PgBanTypes => "MIG-PG-BAN-TYPES",
            Self::D1ForbidTransaction => "MIG-D1-FORBID-TRANSACTION",
            Self::SqliteAutoincrementTrap => "MIG-SQLITE-AUTOINCREMENT-TRAP",
            Self::DestructiveDdl => "MIG-DESTRUCTIVE-DDL",
            Self::VersionCollisionOrGap => "MIG-VERSION-COLLISION-OR-GAP",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::PgConcurrentIndex => {
                "Use `CREATE [UNIQUE] INDEX CONCURRENTLY` and `DROP INDEX CONCURRENTLY` on existing tables to avoid blocking writes."
            }
            Self::PgConcurrentlyInTx => {
                "Remove explicit `BEGIN` / `COMMIT` wrappers around `CONCURRENTLY` index operations."
            }
            Self::PgAddNotNullUnsafe => {
                "Add nullable column or constant/non-volatile `DEFAULT`, backfill in batches, add `CHECK (... IS NOT NULL) NOT VALID`, `VALIDATE CONSTRAINT`, then `SET NOT NULL`."
            }
            Self::PgConstraintNotValid => {
                "Append `NOT VALID` to `FOREIGN KEY`/`CHECK` constraints and run `VALIDATE CONSTRAINT` in a separate transaction; for `UNIQUE`, build a unique index `CONCURRENTLY` and `ADD CONSTRAINT ... UNIQUE USING INDEX`."
            }
            Self::PgAlterColumnType => {
                "Avoid `ALTER COLUMN ... TYPE` rewrites on live tables (only widening `VARCHAR(n)` -> `VARCHAR(m)` or `TEXT` is metadata-only); use expand-and-contract columns instead."
            }
            Self::PgBanTypes => {
                "Use `TEXT` or `VARCHAR` instead of fixed-padding `CHAR(n)`, and `TIMESTAMPTZ` (`TIMESTAMP WITH TIME ZONE`) instead of `TIMESTAMP`."
            }
            Self::D1ForbidTransaction => {
                "Remove `BEGIN`, `COMMIT`, `ROLLBACK`, and `SAVEPOINT` statements; Cloudflare D1 wraps each migration atomically on the server."
            }
            Self::SqliteAutoincrementTrap => {
                "Use `INTEGER PRIMARY KEY` without `AUTOINCREMENT` in SQLite/D1 so rowid allocation avoids `sqlite_sequence` B-tree contention."
            }
            Self::DestructiveDdl => {
                "Use expand-and-contract migrations instead of `DROP TABLE`, `TRUNCATE`, `DROP COLUMN`, or `RENAME` while old application replicas still run."
            }
            Self::VersionCollisionOrGap => {
                "Assign strictly unique sequential migration version prefixes per folder and ensure every `.down.sql` pairs with a matching `.up.sql`."
            }
        }
    }
}
