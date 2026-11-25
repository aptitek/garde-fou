//! Multi-dialect SQL (`PostgreSQL`, `SQLite`, Cloudflare `D1`, `MySQL`, `DuckDB`)
//! AST & zero-downtime migration extractor (`GF000`, `GF070`, `GF080`–`GF081`).

pub mod ast_queries;
pub mod d1_sqlite_migrations;
pub mod dialect_detect;
mod drizzle_config;
pub mod embedded_sql;
mod pg_alter_checks;
pub mod pg_migrations;
pub mod pg_types;
pub mod pg_types_defaults;
pub mod sql_tool_config;
pub mod stmt_splitter;
pub mod token_chunks;

use gf_core::{
    DestructiveDdlFinding, LangRuleFinding, Language, RepoPath, SqlDialectKind, SqlDocument,
    SqlRole, UastDocument,
};

use crate::cst::line_metrics;
use crate::types::ParseError;
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

/// Extracts a `.sql` [`UastDocument`] with an optional workspace-level [`SqlDialectKind`] hint
/// (inferred from `wrangler.toml`, `drizzle.config.*`, or `[sql] dialect`).
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

    let (statements, syntax_errors) = parse_sql_statements(source, dialect);
    doc.syntax_errors = syntax_errors;
    let destructive_ddl =
        evaluate_sql_statements(&statements, dialect, &role, &mut doc.lang_rule_findings);

    doc.lang_rule_findings
        .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
    doc.sql_doc = Some(SqlDocument {
        dialect,
        role,
        migration_group_dir,
        destructive_ddl,
    });
    Ok(doc)
}
