//! Multi-dialect SQL (`PostgreSQL`, `MariaDB`, `Oracle PL/SQL`, `SQLite`, Cloudflare `D1`, `MySQL`, `DuckDB`)
//! AST, procedural unit & zero-downtime migration extractor (`GF000`, `GF070`, `GF076`, `GF080`–`GF081`).

pub mod ast_queries;
pub mod d1_sqlite_migrations;
pub mod db_test_inspector;
pub mod dialect_detect;
mod drizzle_config;
pub mod embedded_sql;
pub mod mariadb;
mod pg_alter_checks;
pub mod pg_migrations;
pub mod pg_types;
pub mod pg_types_defaults;
pub mod plpgsql_checks;
mod plpgsql_extract;
pub mod plsql_ast;
pub mod plsql_checks;
pub mod postgres_rls_sec;
pub mod segmenter;
pub mod sql_tool_config;
pub mod stmt_splitter;
pub mod token_chunks;

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    DestructiveDdlFinding, LangRuleFinding, Language, RepoPath, SqlDialectKind, SqlDocument,
    SqlRole, UastDocument,
};

use crate::{cst::line_metrics, types::ParseError};
use ast_queries::check_universal_sql_statement;
use d1_sqlite_migrations::{DestructiveDdlTracker, check_sqlite_and_d1_statement};
use dialect_detect::{classify_sql_role_and_group, resolve_sql_dialect, scan_sql_bypass_comments};
use pg_migrations::PgMigrationState;
use stmt_splitter::{ParsedSqlStmt, parse_sql_statements};

fn step_destructive_if_up(
    parsed: &ParsedSqlStmt,
    up_mig: bool,
    tracker: &mut DestructiveDdlTracker,
    out: &mut Vec<DestructiveDdlFinding>,
) {
    if up_mig && let ParsedSqlStmt::Ast { stmt, span } = parsed {
        tracker.step(stmt, *span, out);
    }
}

fn evaluate_sql_statements(
    statements: &[ParsedSqlStmt],
    dialect: SqlDialectKind,
    role: &SqlRole,
    findings: &mut Vec<LangRuleFinding>,
) -> Vec<DestructiveDdlFinding> {
    let is_mig = matches!(role, SqlRole::Migration { .. });
    let is_down = matches!(role, SqlRole::Migration { is_down: true, .. });
    let mut pg_state = PgMigrationState::new(is_down);
    let mut destructive_tracker = DestructiveDdlTracker::default();
    let mut destructive_ddl = Vec::new();

    for parsed in statements {
        if let ParsedSqlStmt::Ast { stmt, span } = parsed {
            check_universal_sql_statement(stmt, *span, role, findings);
            check_sqlite_and_d1_statement(stmt, *span, dialect, findings);
        }
        step_destructive_if_up(
            parsed,
            is_mig && !is_down,
            &mut destructive_tracker,
            &mut destructive_ddl,
        );
        if dialect == SqlDialectKind::Postgres && is_mig {
            pg_state.step_statement(parsed, findings);
        }
    }
    destructive_ddl
}

const fn empty_sql_doc(dialect: SqlDialectKind, role: SqlRole, group: String) -> SqlDocument {
    SqlDocument {
        dialect,
        role,
        migration_group_dir: group,
        destructive_ddl: Vec::new(),
        rls_enabled_tables: BTreeSet::new(),
        rls_forced_tables: BTreeSet::new(),
        rls_policies_by_table: BTreeMap::new(),
        plsql_package_specs: BTreeMap::new(),
        plsql_package_bodies: BTreeMap::new(),
    }
}

fn extract_plsql_into(source: &str, doc: &mut UastDocument, sql_doc: &mut SqlDocument) {
    let segments = segmenter::segment_oracle_slash_units(source);
    let units = plsql_ast::parse_plsql_units(&segments, sql_doc, &mut doc.callables);
    plsql_checks::analyze_plsql_units(&units, &mut doc.lang_rule_findings);
}

/// Extracts an Oracle PL/SQL [`UastDocument`] (`.pks`, `.pkb`, `.pls`, `.plb`, `.prc`, `.fnc`, `.trg`, `.tps`, `.tpb`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document root span fails.
pub fn extract_plsql(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), Language::PlSql, total_lines, effective_lines)?;
    scan_sql_bypass_comments(source, &mut doc.bypass_findings);
    let mut sql_doc = empty_sql_doc(
        SqlDialectKind::OraclePlSql,
        SqlRole::QueryOrSchema,
        String::new(),
    );
    extract_plsql_into(source, &mut doc, &mut sql_doc);
    db_test_inspector::extract_db_test_cases(path, source, &mut doc.test_cases);
    doc.lang_rule_findings
        .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
    doc.sql_doc = Some(sql_doc);
    Ok(doc)
}

const fn mariadb_mode(role: &SqlRole) -> mariadb::MariaDbMode {
    match role {
        SqlRole::Migration { is_down: false, .. } => mariadb::MariaDbMode::UpMigration,
        SqlRole::Migration { is_down: true, .. } => mariadb::MariaDbMode::DownMigration,
        SqlRole::QueryOrSchema => mariadb::MariaDbMode::Query,
    }
}

fn analyze_standard_sql(
    source: &str,
    dialect: SqlDialectKind,
    doc: &mut UastDocument,
    sql_doc: &mut SqlDocument,
) {
    let (statements, mut syntax_errors) = parse_sql_statements(source, dialect);
    if dialect == SqlDialectKind::MariaDb {
        syntax_errors.retain(|e| {
            !e.message.contains("found: ALGORITHM") && !e.message.contains("found: LOCK")
        });
    }
    doc.syntax_errors = syntax_errors;
    sql_doc.destructive_ddl = evaluate_sql_statements(
        &statements,
        dialect,
        &sql_doc.role,
        &mut doc.lang_rule_findings,
    );
    if dialect == SqlDialectKind::MariaDb {
        let segs = segmenter::segment_mariadb_delimiters(source);
        mariadb::analyze_mariadb_segments(
            &segs,
            mariadb_mode(&sql_doc.role),
            &mut doc.lang_rule_findings,
        );
    } else if dialect == SqlDialectKind::Postgres {
        let is_down = matches!(sql_doc.role, SqlRole::Migration { is_down: true, .. });
        postgres_rls_sec::extract_pg_rls_and_sec_definer(
            source,
            is_down,
            sql_doc,
            &mut doc.lang_rule_findings,
        );
        plpgsql_checks::analyze_plpgsql_source(
            source,
            &mut doc.callables,
            &mut doc.lang_rule_findings,
        );
    }
}

/// Extracts a `.sql` [`UastDocument`] with an optional workspace-level [`SqlDialectKind`] hint.
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document root span fails.
pub fn extract_sql_with_dialect_hint(
    path: &RepoPath,
    source: &str,
    ws_hint: Option<SqlDialectKind>,
) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), Language::Sql, total_lines, effective_lines)?;
    let (role, migration_group_dir) = classify_sql_role_and_group(path);
    let dialect = resolve_sql_dialect(source, ws_hint);
    scan_sql_bypass_comments(source, &mut doc.bypass_findings);
    let mut sql_doc = empty_sql_doc(dialect, role.clone(), migration_group_dir);

    if dialect == SqlDialectKind::OraclePlSql {
        extract_plsql_into(source, &mut doc, &mut sql_doc);
    } else if dialect == SqlDialectKind::MariaDb
        && source.to_ascii_uppercase().contains("DELIMITER ")
    {
        let segs = segmenter::segment_mariadb_delimiters(source);
        mariadb::analyze_mariadb_segments(&segs, mariadb_mode(&role), &mut doc.lang_rule_findings);
    } else {
        analyze_standard_sql(source, dialect, &mut doc, &mut sql_doc);
    }

    db_test_inspector::extract_db_test_cases(path, source, &mut doc.test_cases);
    doc.lang_rule_findings
        .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
    doc.sql_doc = Some(sql_doc);
    Ok(doc)
}
