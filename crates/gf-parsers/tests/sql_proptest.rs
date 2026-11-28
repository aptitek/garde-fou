//! Property-based tests (`proptest`) for multi-dialect SQL parsing, dollar-quoting,
//! `PostgreSQL` zero-downtime state invariants, and `squawk` 0-based JSON normalization.

use std::path::Path;

use gf_core::{Config, RepoPath, SqlDialectKind};
use gf_parsers::{parse_source_with_context, parse_squawk_json};
use gf_rules::evaluate_document;
use proptest::prelude::*;
use proptest::test_runner::{Config as PropConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(PropConfig {
        cases: 192,
        failure_persistence: None,
        ..PropConfig::default()
    })
}

#[test]
fn arbitrary_sql_dollar_quotes_and_triggers_never_panic_across_dialects() -> Result<(), String> {
    let mut tr = runner();
    let cfg = Config::strict_defaults();
    let dialects = [
        SqlDialectKind::Postgres,
        SqlDialectKind::Sqlite,
        SqlDialectKind::CloudflareD1,
        SqlDialectKind::MySql,
        SqlDialectKind::DuckDb,
        SqlDialectKind::Generic,
    ];
    let strat = (
        (
            0usize..dialects.len(),
            "[A-Za-z0-9_;$'\"()=<>!*+\\-\n ]{0,220}",
        ),
        prop::bool::ANY,
    );
    tr.run(&strat, |((d_idx, raw_sql), is_mig)| {
        let path_str = if is_mig {
            "migrations/0001_fuzz.sql"
        } else {
            "sql/fuzz.sql"
        };
        let path = RepoPath::new(path_str).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = parse_source_with_context(&path, &raw_sql, &cfg.domains, Some(dialects[d_idx]))
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(doc.root.span.start_line(), 1);
        prop_assert!(doc.effective_lines <= doc.total_lines);
        for diag in evaluate_document(&doc, &cfg) {
            prop_assert!(diag.span.start_line() >= 1);
            prop_assert!(diag.span.end_line() >= diag.span.start_line());
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn eval_pg_migration(sql: &str, cfg: &Config) -> Result<Vec<gf_core::Diagnostic>, TestCaseError> {
    let path =
        RepoPath::new("migrations/0010_idx.sql").map_err(|e| TestCaseError::fail(e.to_string()))?;
    let doc = parse_source_with_context(&path, sql, &cfg.domains, Some(SqlDialectKind::Postgres))
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    Ok(evaluate_document(&doc, cfg))
}

#[test]
fn postgres_index_concurrency_satisfies_new_vs_existing_table_invariant() -> Result<(), String> {
    let mut tr = runner();
    let cfg = Config::strict_defaults();
    let ident = "[a-z][a-z0-9_]{2,10}".prop_filter("avoid SQL keywords", |s| {
        !matches!(
            s.as_str(),
            "select"
                | "table"
                | "index"
                | "where"
                | "order"
                | "group"
                | "user"
                | "check"
                | "default"
        )
    });
    let strat = ((ident.clone(), ident.clone()), (ident, prop::bool::ANY));

    tr.run(&strat, |((tbl, col), (idx, concurrently))| {
        let conc_kw = if concurrently { "CONCURRENTLY " } else { "" };
        let new_tbl_sql = format!(
            "CREATE TABLE {tbl} ({col} BIGINT NOT NULL);\nCREATE INDEX {idx} ON {tbl} ({col});\n"
        );
        prop_assert_eq!(eval_pg_migration(&new_tbl_sql, &cfg)?, Vec::new());

        let existing_sql = format!("CREATE INDEX {conc_kw}{idx} ON {tbl} ({col});\n");
        let diags = eval_pg_migration(&existing_sql, &cfg)?;
        if concurrently {
            prop_assert_eq!(diags, Vec::new());
        } else {
            prop_assert_eq!(diags.len(), 1);
            prop_assert!(diags[0].message.contains("MIG-PG-CONCURRENT-INDEX"));
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn squawk_json_0based_coordinates_always_shift_to_1based() -> Result<(), String> {
    let mut tr = runner();
    tr.run(&(0u32..50_000, 0u32..5_000), |(raw_line, raw_col)| {
        let payload = format!(
            "[{{\"file\":\"migrations/001.sql\",\"line\":{raw_line},\"column\":{raw_col},\"rule_name\":\"adding-not-null-field\",\"message\":\"unsafe\"}}]"
        );
        let findings = parse_squawk_json(&payload, Path::new("/repo"))
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(findings.len(), 1);
        prop_assert_eq!(findings[0].line, raw_line + 1);
        prop_assert_eq!(findings[0].column, raw_col + 1);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
