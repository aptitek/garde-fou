//! Closed algebraic sum types for MariaDB (`MARIADB-*`), advanced `PostgreSQL`/PL/pgSQL
//! (`PG-*`, `PLPGSQL-*`), and Oracle PL/SQL (`PLSQL-*`) procedural rules (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// MariaDB 10.11 / 11.4 DDL, charset, engine, and transaction violations (`MARIADB-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MariaDbViolation {
    /// DDL (`CREATE`/`ALTER`/`DROP`/`TRUNCATE`/`RENAME`) inside an explicit `START TRANSACTION`/`BEGIN` block causing an implicit commit.
    ImplicitDdlCommit,
    /// Legacy 3-byte `utf8` / `utf8mb3` charset or collation instead of `utf8mb4`.
    Utf8mb3Trap,
    /// `ALTER TABLE` on an existing table using `ALGORITHM=COPY`, `LOCK=EXCLUSIVE`/`SHARED`, or missing online `ALGORITHM=INSTANT|INPLACE, LOCK=NONE`.
    BlockingAlterDdl,
    /// Non-ACID storage engine (`MyISAM`/`MEMORY`/`ARIA`), disabled `CHECK ... NOT ENFORCED`, or `LONGTEXT` JSON column lacking `JSON_VALID(...)`.
    NonAcidEngineAndJson,
}

impl MariaDbViolation {
    /// Canonical `MARIADB-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "MARIADB-IMPLICIT-DDL-COMMIT",
            "MARIADB-UTF8MB3-TRAP",
            "MARIADB-BLOCKING-ALTER-DDL",
            "MARIADB-NON-ACID-ENGINE-AND-JSON",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 4] = [
            "Move DDL statements out of explicit MariaDB transactions so they never trigger an irreversible implicit COMMIT.",
            "Replace `utf8` / `utf8mb3` with `CHARACTER SET utf8mb4 COLLATE utf8mb4_uca1400_ai_ci`.",
            "Specify `ALGORITHM=INSTANT` or `ALGORITHM=INPLACE, LOCK=NONE` on MariaDB `ALTER TABLE` migrations against existing tables.",
            "Use `ENGINE=InnoDB`, keep `CHECK` constraints `ENFORCED`, and declare `JSON` (or `CHECK (JSON_VALID(col))`) for JSON columns.",
        ];
        REM[self as usize]
    }
}

/// Advanced `PostgreSQL` RLS, `SECURITY DEFINER`, and PL/pgSQL violations (`PG-*`, `PLPGSQL-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PostgresAdvViolation {
    /// Table has `ENABLE ROW LEVEL SECURITY` with zero `CREATE POLICY` across the workspace (silent deny-all lock).
    RlsWithoutPolicy,
    /// Table has `CREATE POLICY` without `ENABLE ROW LEVEL SECURITY` across the workspace (critical bypass!).
    PolicyWithoutRls,
    /// `SECURITY DEFINER` function/procedure missing an explicit safe `SET search_path = pg_catalog, pg_temp` (or `''`).
    SecurityDefinerSearchPath,
    /// PL/pgSQL `SELECT ... INTO v` without `STRICT` (nor guaranteed scalar aggregate / `LIMIT 1`).
    PlpgsqlMissingIntoStrict,
    /// PL/pgSQL `EXECUTE '...' || var` or `EXECUTE format('... %s ...', ...)` allowing SQL injection.
    PlpgsqlUnsafeDynamicExecute,
}

impl PostgresAdvViolation {
    /// Canonical `PG-*` / `PLPGSQL-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "PG-RLS-WITHOUT-POLICY",
            "PG-POLICY-WITHOUT-RLS",
            "PG-SECURITY-DEFINER-SEARCH-PATH",
            "PLPGSQL-MISSING-INTO-STRICT",
            "PLPGSQL-UNSAFE-DYNAMIC-EXECUTE",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 5] = [
            "Add at least one explicit `CREATE POLICY ... ON <table>` after enabling Row Level Security.",
            "Run `ALTER TABLE <table> ENABLE ROW LEVEL SECURITY` so PostgreSQL enforces declared RLS policies.",
            "Attach `SET search_path = pg_catalog, pg_temp` (or `SET search_path = ''`) directly on every `SECURITY DEFINER` routine.",
            "Use `SELECT ... INTO STRICT <var>` (with `NO_DATA_FOUND`/`TOO_MANY_ROWS` handling) or an explicit `LIMIT 1` / scalar aggregate.",
            "Use `EXECUTE format('... %I ... $1', ident) USING val` instead of string concatenation `||` or raw `%s`.",
        ];
        REM[self as usize]
    }
}

/// Oracle PL/SQL procedural & package governance violations (`PLSQL-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ProceduralSqlViolation {
    /// Procedure or function declared in `CREATE PACKAGE` missing from `CREATE PACKAGE BODY`.
    PackageSpecMismatch,
    /// Dynamic SQL (`EXECUTE IMMEDIATE` / `OPEN ... FOR` / `DBMS_SQL.PARSE`) built with `||` without `:1` + `USING` / `DBMS_ASSERT`.
    ExecuteImmediateInjection,
    /// `WHEN OTHERS THEN` handler swallowing exceptions without `RAISE;` or `RAISE_APPLICATION_ERROR(...)`.
    SwallowedWhenOthers,
    /// Row-by-row DML (`INSERT`/`UPDATE`/`DELETE`/`MERGE`) inside `LOOP ... END LOOP` instead of `BULK COLLECT` + `FORALL`.
    DmlInLoop,
    /// `COMMIT` or `ROLLBACK` inside a `LOOP` or inside a `TRIGGER` lacking `PRAGMA AUTONOMOUS_TRANSACTION`.
    CommitInLoop,
    /// `FETCH <cur> BULK COLLECT INTO ...` missing `LIMIT <n>` (PGA memory exhaustion risk).
    BulkCollectNoLimit,
    /// Explicit `OPEN <cur>` lacking a matching `CLOSE <cur>` in the unit.
    UnclosedCursor,
    /// `PRAGMA AUTONOMOUS_TRANSACTION` unit without an explicit `COMMIT` or `ROLLBACK` (`ORA-06519`).
    AutonomousTxUnterminated,
    /// `CREATE [OR REPLACE] PACKAGE|PROCEDURE|FUNCTION` omitting `AUTHID CURRENT_USER` (and not explicitly justified `AUTHID DEFINER`).
    MissingAuthidCurrentUser,
}

impl ProceduralSqlViolation {
    /// Canonical `PLSQL-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 9] = [
            "PLSQL-PACKAGE-SPEC-MISMATCH",
            "PLSQL-EXECUTE-IMMEDIATE-INJECTION",
            "PLSQL-SWALLOWED-WHEN-OTHERS",
            "PLSQL-DML-IN-LOOP",
            "PLSQL-COMMIT-IN-LOOP",
            "PLSQL-BULK-COLLECT-NO-LIMIT",
            "PLSQL-UNCLOSED-CURSOR",
            "PLSQL-AUTONOMOUS-TX-UNTERMINATED",
            "PLSQL-MISSING-AUTHID-CURRENT-USER",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 9] = [
            "Implement every procedure and function declared in `CREATE PACKAGE` inside its matching `CREATE PACKAGE BODY`.",
            "Use bind variables (`:1`) with `USING` (and `DBMS_ASSERT.SIMPLE_SQL_NAME`/`ENQUOTE_NAME` for identifiers) instead of `||` concatenation.",
            "Re-raise errors in `WHEN OTHERS` via `RAISE;` or `RAISE_APPLICATION_ERROR(...)` after logging.",
            "Replace row-by-row DML inside `LOOP` with `FETCH ... BULK COLLECT INTO ... LIMIT n` and `FORALL ... SAVE EXCEPTIONS`.",
            "Move `COMMIT`/`ROLLBACK` outside loops and triggers (or declare `PRAGMA AUTONOMOUS_TRANSACTION` for audit triggers).",
            "Add `LIMIT <n>` (e.g. `LIMIT 500`) to `FETCH ... BULK COLLECT INTO` to bound PGA memory usage.",
            "Close every explicitly opened cursor via `CLOSE <cursor>;` (also on exception paths) or prefer `FOR rec IN cur LOOP`.",
            "Terminate every `PRAGMA AUTONOMOUS_TRANSACTION` block with an explicit `COMMIT;` or `ROLLBACK;` to prevent `ORA-06519`.",
            "Declare `AUTHID CURRENT_USER` on application PL/SQL packages, procedures, and functions to enforce invoker rights.",
        ];
        REM[self as usize]
    }
}

/// Unified closed sum type across MariaDB, advanced `PostgreSQL`/PL/pgSQL, and Oracle PL/SQL (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DbProceduralViolation {
    /// MariaDB 10.11 / 11.4 violation (`MARIADB-*`).
    MariaDb(MariaDbViolation),
    /// Advanced `PostgreSQL` / PL/pgSQL violation (`PG-*`, `PLPGSQL-*`).
    PostgresAdv(PostgresAdvViolation),
    /// Oracle PL/SQL procedural violation (`PLSQL-*`).
    ProceduralSql(ProceduralSqlViolation),
}

impl DbProceduralViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MariaDb(v) => v.code(),
            Self::PostgresAdv(v) => v.code(),
            Self::ProceduralSql(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::MariaDb(v) => v.remediation(),
            Self::PostgresAdv(v) => v.remediation(),
            Self::ProceduralSql(v) => v.remediation(),
        }
    }
}

impl fmt::Display for DbProceduralViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
