//! Unit tests verifying MariaDB, advanced PostgreSQL/PL/pgSQL, and Oracle PL/SQL violation metadata.

use gf_core::{
    DbProceduralViolation, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation,
};

#[rustfmt::skip]
const CASES: &[(DbProceduralViolation, &str, &str)] = &[
    (DbProceduralViolation::MariaDb(MariaDbViolation::ImplicitDdlCommit), "MARIADB-IMPLICIT-DDL-COMMIT", "COMMIT"),
    (DbProceduralViolation::MariaDb(MariaDbViolation::Utf8mb3Trap), "MARIADB-UTF8MB3-TRAP", "utf8mb4"),
    (DbProceduralViolation::MariaDb(MariaDbViolation::BlockingAlterDdl), "MARIADB-BLOCKING-ALTER-DDL", "ALGORITHM=INSTANT"),
    (DbProceduralViolation::MariaDb(MariaDbViolation::NonAcidEngineAndJson), "MARIADB-NON-ACID-ENGINE-AND-JSON", "InnoDB"),
    (DbProceduralViolation::PostgresAdv(PostgresAdvViolation::RlsWithoutPolicy), "PG-RLS-WITHOUT-POLICY", "CREATE POLICY"),
    (DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PolicyWithoutRls), "PG-POLICY-WITHOUT-RLS", "ENABLE ROW LEVEL SECURITY"),
    (DbProceduralViolation::PostgresAdv(PostgresAdvViolation::SecurityDefinerSearchPath), "PG-SECURITY-DEFINER-SEARCH-PATH", "search_path"),
    (DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PlpgsqlMissingIntoStrict), "PLPGSQL-MISSING-INTO-STRICT", "INTO STRICT"),
    (DbProceduralViolation::PostgresAdv(PostgresAdvViolation::PlpgsqlUnsafeDynamicExecute), "PLPGSQL-UNSAFE-DYNAMIC-EXECUTE", "USING"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::PackageSpecMismatch), "PLSQL-PACKAGE-SPEC-MISMATCH", "PACKAGE BODY"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::ExecuteImmediateInjection), "PLSQL-EXECUTE-IMMEDIATE-INJECTION", "DBMS_ASSERT"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::SwallowedWhenOthers), "PLSQL-SWALLOWED-WHEN-OTHERS", "RAISE"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::DmlInLoop), "PLSQL-DML-IN-LOOP", "FORALL"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::CommitInLoop), "PLSQL-COMMIT-IN-LOOP", "AUTONOMOUS_TRANSACTION"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::BulkCollectNoLimit), "PLSQL-BULK-COLLECT-NO-LIMIT", "LIMIT"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::UnclosedCursor), "PLSQL-UNCLOSED-CURSOR", "CLOSE"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::AutonomousTxUnterminated), "PLSQL-AUTONOMOUS-TX-UNTERMINATED", "ORA-06519"),
    (DbProceduralViolation::ProceduralSql(ProceduralSqlViolation::MissingAuthidCurrentUser), "PLSQL-MISSING-AUTHID-CURRENT-USER", "AUTHID CURRENT_USER"),
];

#[test]
fn db_procedural_violations_expose_exact_codes_display_and_remediation() {
    for &(v, code, needle) in CASES {
        assert_eq!(v.code(), code);
        assert_eq!(format!("{v}"), code);
        assert!(v.remediation().contains(needle));
    }
}
