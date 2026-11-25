//! Sequential `PostgreSQL` zero-downtime migration state machine (`squawk`-grade `GF080`).

use gf_core::{LangRuleFinding, MigrationViolation, Span};
use sqlparser::ast::{CreateIndex, CreateTable, ObjectType, Statement};

use super::pg_alter_checks::PgCatalogState;
use super::pg_types_defaults::{
    check_pg_banned_data_type, normalize_ident_key, normalize_table_key,
};
use super::stmt_splitter::ParsedSqlStmt;

/// Sequential `PostgreSQL` migration state tracked across statements in a `.sql` file.
#[derive(Default)]
pub struct PgMigrationState {
    is_down: bool,
    cat: PgCatalogState,
}

impl PgMigrationState {
    /// Creates a new state machine (`is_down = true` suppresses lock/rewrite warnings while keeping `CONCURRENTLY` tx errors).
    #[must_use]
    pub fn new(is_down: bool) -> Self {
        Self {
            is_down,
            cat: PgCatalogState::default(),
        }
    }

    fn enter_tx(&mut self) {
        self.cat.in_explicit_tx = true;
        self.cat.tx_created_tables.clear();
        self.cat.not_valid_in_tx.clear();
    }

    fn commit_tx(&mut self) {
        self.cat.in_explicit_tx = false;
        self.cat.tx_created_tables.clear();
        self.cat.not_valid_in_tx.clear();
    }

    fn rollback_tx(&mut self) {
        for tbl in &self.cat.tx_created_tables {
            self.cat.created_tables.remove(tbl);
        }
        self.cat.in_explicit_tx = false;
        self.cat.tx_created_tables.clear();
        self.cat.not_valid_in_tx.clear();
    }

    fn flag_concurrently_if_in_tx(
        &self,
        op_label: &str,
        span: Span,
        out: &mut Vec<LangRuleFinding>,
    ) {
        if self.cat.in_explicit_tx {
            out.push(LangRuleFinding::new(
                MigrationViolation::PgConcurrentlyInTx,
                span,
                format!(
                    "`{op_label}` cannot execute inside an explicit `BEGIN ... COMMIT` transaction block"
                ),
            ));
        }
    }

    fn record_create_table(
        &mut self,
        ct: &CreateTable,
        span: Span,
        out: &mut Vec<LangRuleFinding>,
    ) {
        let tbl = normalize_table_key(&ct.name);
        self.cat.created_tables.insert(tbl.clone());
        if self.cat.in_explicit_tx {
            self.cat.tx_created_tables.insert(tbl.clone());
        }
        if !self.is_down {
            for col in &ct.columns {
                check_pg_banned_data_type(&col.name.value, &col.data_type, span, out);
                self.cat.col_types.insert(
                    (tbl.clone(), normalize_ident_key(&col.name.value)),
                    col.data_type.clone(),
                );
            }
        }
    }

    fn record_create_index(&self, ci: &CreateIndex, span: Span, out: &mut Vec<LangRuleFinding>) {
        if ci.concurrently {
            self.flag_concurrently_if_in_tx("CREATE INDEX CONCURRENTLY", span, out);
        } else if !self.is_down {
            let tbl = normalize_table_key(&ci.table_name);
            if !self.cat.created_tables.contains(&tbl) {
                out.push(LangRuleFinding::new(
                    MigrationViolation::PgConcurrentIndex,
                    span,
                    format!(
                        "`CREATE INDEX` without `CONCURRENTLY` on existing table `{tbl}` blocks concurrent writes"
                    ),
                ));
            }
        }
    }

    /// Steps the `PostgreSQL` migration state machine on `parsed`.
    pub fn step_statement(&mut self, parsed: &ParsedSqlStmt, out: &mut Vec<LangRuleFinding>) {
        match parsed {
            ParsedSqlStmt::DropIndexConcurrently { span } => {
                self.flag_concurrently_if_in_tx("DROP INDEX CONCURRENTLY", *span, out);
            }
            ParsedSqlStmt::Reindex { concurrently, span } if *concurrently => {
                self.flag_concurrently_if_in_tx("REINDEX CONCURRENTLY", *span, out);
            }
            ParsedSqlStmt::Reindex { .. } => {}
            ParsedSqlStmt::Ast { stmt, span } => self.step_ast_stmt(stmt, *span, out),
        }
    }

    fn step_ast_stmt(&mut self, stmt: &Statement, span: Span, out: &mut Vec<LangRuleFinding>) {
        match stmt {
            Statement::StartTransaction { .. } => self.enter_tx(),
            Statement::Commit { .. } => self.commit_tx(),
            Statement::Rollback {
                savepoint: None, ..
            } => self.rollback_tx(),
            Statement::CreateTable(ct) => self.record_create_table(ct, span, out),
            Statement::CreateIndex(ci) => self.record_create_index(ci, span, out),
            Statement::Drop {
                object_type: ObjectType::Index,
                ..
            } if !self.is_down => {
                out.push(LangRuleFinding::new(
                    MigrationViolation::PgConcurrentIndex,
                    span,
                    "`DROP INDEX` without `CONCURRENTLY` acquires `ACCESS EXCLUSIVE` lock; use `DROP INDEX CONCURRENTLY`",
                ));
            }
            Statement::AlterTable(alter) if !self.is_down => {
                self.cat.evaluate_alter_table(alter, span, out);
            }
            _ => {}
        }
    }
}
