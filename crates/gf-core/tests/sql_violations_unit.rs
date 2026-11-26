//! Unit tests for `#![no_std]` SQL & migration sum types, dialect settings, and tool configs.

use gf_core::{
    Config, DrizzleConfigIssue, ExternalToolKind, LangRuleFinding, LangRuleViolation,
    MigrationViolation, RepoPath, Span, SqlConfig, SqlDialectKind, SqlDialectSetting,
    SqlToolConfigIssue, SqlToolsConfig, SqlViolation, SqruffSqlfluffConfigIssue, SquawkConfigIssue,
    ToolConfigFinding, ToolConfigViolation, is_audited_tool_config_file, is_sql_tool_config_file,
};

#[test]
fn sql_dialect_and_setting_roundtrip_and_cache_fragments_are_distinct() {
    #[rustfmt::skip]
    let dialect_cases = [
        ("postgres", SqlDialectKind::Postgres, "postgres"), ("PostgreSQL", SqlDialectKind::Postgres, "postgres"),
        ("pg", SqlDialectKind::Postgres, "postgres"), ("sqlite", SqlDialectKind::Sqlite, "sqlite"),
        ("sqlite3", SqlDialectKind::Sqlite, "sqlite"), ("d1", SqlDialectKind::CloudflareD1, "d1"),
        ("cloudflare-d1", SqlDialectKind::CloudflareD1, "d1"), ("mysql", SqlDialectKind::MySql, "mysql"),
        ("mariadb", SqlDialectKind::MariaDb, "mariadb"), ("plsql", SqlDialectKind::OraclePlSql, "plsql"),
        ("oracle", SqlDialectKind::OraclePlSql, "plsql"), ("duckdb", SqlDialectKind::DuckDb, "duckdb"),
        ("generic", SqlDialectKind::Generic, "generic"), ("ansi", SqlDialectKind::Generic, "generic"),
    ];
    for (raw, expected, canon) in dialect_cases {
        assert_eq!(SqlDialectKind::from_config_str(raw), Some(expected));
        assert_eq!(expected.as_str(), canon);
        assert_eq!(expected.to_string(), canon);
        assert_eq!(
            SqlDialectSetting::from_config_str(raw),
            Some(SqlDialectSetting::Explicit(expected))
        );
    }
    assert_eq!(SqlDialectKind::from_config_str("unknown-db"), None);
    assert_eq!(
        SqlDialectSetting::from_config_str("AUTO"),
        Some(SqlDialectSetting::Auto)
    );
    assert_eq!(SqlDialectSetting::Auto.as_str(), "auto");
    assert_eq!(
        SqlDialectSetting::Explicit(SqlDialectKind::CloudflareD1).as_str(),
        "d1"
    );

    let c1 = Config::strict_defaults();
    let mut c2 = Config::strict_defaults();
    c2.sql = SqlConfig {
        dialect: SqlDialectSetting::Explicit(SqlDialectKind::CloudflareD1),
        allow_destructive_migrations: true,
    };
    assert_ne!(c1.canonical_string(), c2.canonical_string());
    assert_ne!(
        SqlToolsConfig::defaults().canonical_fragment(),
        SqlToolsConfig {
            squawk: gf_core::ScannerPolicy::Required,
            ..SqlToolsConfig::defaults()
        }
        .canonical_fragment()
    );
}

#[test]
fn sql_violation_codes_and_remediations_are_exact() {
    for (v, code) in [
        (SqlViolation::NullComparison, "SQL-NULL-COMPARISON"),
        (SqlViolation::NotInSubquery, "SQL-NOT-IN-SUBQUERY"),
        (SqlViolation::UnboundedMutation, "SQL-UNBOUNDED-MUTATION"),
        (SqlViolation::SelectStar, "SQL-SELECT-STAR"),
        (
            SqlViolation::InjectionInterpolation,
            "SQL-INJECTION-INTERPOLATION",
        ),
    ] {
        let f = LangRuleFinding::new(v, Span::FIRST, "sql issue");
        assert_eq!(f.code(), code);
        assert!(f.remediation().len() >= 25, "{}", f.remediation());
        assert!(!LangRuleViolation::from(v).is_entrypoint_exempt());
    }
    assert!(
        SqlViolation::NullComparison
            .remediation()
            .contains("IS NULL")
    );
}

#[test]
fn migration_violation_codes_and_remediations_are_exact() {
    for (v, code) in [
        (
            MigrationViolation::PgConcurrentIndex,
            "MIG-PG-CONCURRENT-INDEX",
        ),
        (
            MigrationViolation::PgConcurrentlyInTx,
            "MIG-PG-CONCURRENTLY-IN-TX",
        ),
        (
            MigrationViolation::PgAddNotNullUnsafe,
            "MIG-PG-ADD-NOT-NULL-UNSAFE",
        ),
        (
            MigrationViolation::PgConstraintNotValid,
            "MIG-PG-CONSTRAINT-NOT-VALID",
        ),
        (
            MigrationViolation::PgAlterColumnType,
            "MIG-PG-ALTER-COLUMN-TYPE",
        ),
        (MigrationViolation::PgBanTypes, "MIG-PG-BAN-TYPES"),
        (
            MigrationViolation::D1ForbidTransaction,
            "MIG-D1-FORBID-TRANSACTION",
        ),
        (
            MigrationViolation::SqliteAutoincrementTrap,
            "MIG-SQLITE-AUTOINCREMENT-TRAP",
        ),
        (MigrationViolation::DestructiveDdl, "MIG-DESTRUCTIVE-DDL"),
        (
            MigrationViolation::VersionCollisionOrGap,
            "MIG-VERSION-COLLISION-OR-GAP",
        ),
    ] {
        let f = LangRuleFinding::new(v, Span::FIRST, "mig issue");
        assert_eq!(f.code(), code);
        assert!(f.remediation().len() >= 25, "{}", f.remediation());
        assert!(!LangRuleViolation::from(v).is_entrypoint_exempt());
    }
    assert!(
        MigrationViolation::PgConcurrentIndex
            .remediation()
            .contains("CONCURRENTLY")
    );
}

#[test]
fn sql_tool_config_issues_map_to_expected_tools_and_keys() {
    for (issue, tool, key) in [
        (
            SqlToolConfigIssue::Squawk(SquawkConfigIssue::CriticalRuleExcluded),
            ExternalToolKind::Squawk,
            "excluded_rules",
        ),
        (
            SqlToolConfigIssue::Squawk(SquawkConfigIssue::BlanketExcludedPaths),
            ExternalToolKind::Squawk,
            "excluded_paths",
        ),
        (
            SqlToolConfigIssue::Sqruff(SqruffSqlfluffConfigIssue::ExcludeAllRules),
            ExternalToolKind::Sqruff,
            "sqlfluff.exclude_rules",
        ),
        (
            SqlToolConfigIssue::Sqlfluff(SqruffSqlfluffConfigIssue::CriticalRuleExcluded),
            ExternalToolKind::Sqlfluff,
            "sqlfluff.exclude_rules",
        ),
        (
            SqlToolConfigIssue::Drizzle(DrizzleConfigIssue::HardcodedDatabaseCredentials),
            ExternalToolKind::Sqruff,
            "dbCredentials.url",
        ),
    ] {
        let f = ToolConfigFinding {
            violation: ToolConfigViolation::Sql(issue),
            span: Span::FIRST,
            message: String::from("weak"),
        };
        assert_eq!(f.tool(), tool);
        assert_eq!(f.key(), key);
        assert!(f.remediation().len() >= 25, "{}", f.remediation());
    }
}

#[test]
fn sql_tool_config_filenames_are_recognized() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        ".squawk.toml",
        "squawk.toml",
        ".sqruff",
        ".sqlfluff",
        "drizzle.config.ts",
    ] {
        let p = RepoPath::new(name)?;
        assert!(is_sql_tool_config_file(&p), "{name}");
        assert!(is_audited_tool_config_file(&p), "{name}");
    }
    assert!(!is_sql_tool_config_file(&RepoPath::new(
        "migrations/001.sql"
    )?));
    Ok(())
}
