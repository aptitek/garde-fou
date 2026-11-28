//! Unit tests verifying MariaDB, advanced PostgreSQL/PL/pgSQL, and Oracle PL/SQL violation metadata.

use gf_core::{
    DbProceduralViolation, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation,
};

struct Case {
    violation: DbProceduralViolation,
    code: &'static str,
    needle: &'static str,
}

const fn case(violation: DbProceduralViolation, code: &'static str, needle: &'static str) -> Case {
    Case {
        violation,
        code,
        needle,
    }
}

#[rustfmt::skip]
const CASES: &[Case] = &[
    case(DbProceduralViolation::MariaDb(MariaDbViolation::ImplicitDdlCommit), "MARIADB-IMPLICIT-DDL-COMMIT", "COMMIT"),
    case(DbProceduralViolation::MariaDb(MariaDbViolation::Utf8mb3Trap), "MARIADB-UTF8MB3-TRAP", "utf8mb4"),
    case(DbProceduralViolation::MariaDb(MariaDbViolation::BlockingAlterDdl), "MARIADB-BLOCKING-ALTER-DDL", "ALGORITHM=INSTANT"),
    case(DbProceduralViolation::MariaDb(MariaDbViolation::NonAcidEngineAndJson), "MARIADB-NON-ACID-ENGINE-AND-JSON", "InnoDB"),
    case(DbProceduralViolation::PostgresAdv(PostgresAdvViolation::RlsWithoutPolicy), "PG-RLS-WITHOUT-POLICY", "CREATE POLICY"),
    case(DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PolicyWithoutRls), "PG-POLICY-WITHOUT-RLS", "ENABLE ROW LEVEL SECURITY"),
    case(DbProceduralViolation::PostgresAdv(PostgresAdvViolation::SecurityDefinerSearchPath), "PG-SECURITY-DEFINER-SEARCH-PATH", "search_path"),
    case(DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PlpgsqlMissingIntoStrict), "PLPGSQL-MISSING-INTO-STRICT", "INTO STRICT"),
    case(DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PlpgsqlUnsafeDynamicExecute), "PLPGSQL-UNSAFE-DYNAMIC-EXECUTE", "USING"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::PackageSpecMismatch), "PLSQL-PACKAGE-SPEC-MISMATCH", "PACKAGE BODY"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::ExecuteImmediateInjection), "PLSQL-EXECUTE-IMMEDIATE-INJECTION", "DBMS_ASSERT"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::SwallowedWhenOthers), "PLSQL-SWALLOWED-WHEN-OTHERS", "RAISE"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::DmlInLoop), "PLSQL-DML-IN-LOOP", "FORALL"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::CommitInLoop), "PLSQL-COMMIT-IN-LOOP", "AUTONOMOUS_TRANSACTION"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::BulkCollectNoLimit), "PLSQL-BULK-COLLECT-NO-LIMIT", "LIMIT"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::UnclosedCursor), "PLSQL-UNCLOSED-CURSOR", "CLOSE"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::AutonomousTxUnterminated), "PLSQL-AUTONOMOUS-TX-UNTERMINATED", "ORA-06519"),
    case(DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::MissingAuthidCurrentUser), "PLSQL-MISSING-AUTHID-CURRENT-USER", "AUTHID CURRENT_USER"),
];

#[test]
fn db_procedural_violations_expose_exact_codes_display_and_remediation() {
    for c in CASES {
        assert_eq!(c.violation.code(), c.code);
        assert_eq!(format!("{}", c.violation), c.code);
        assert!(c.violation.remediation().contains(c.needle));
    }
}
