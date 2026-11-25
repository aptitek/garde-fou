//! `SQLite` / Cloudflare `D1` migration rules (`MIG-D1-FORBID-TRANSACTION`,
//! `MIG-SQLITE-AUTOINCREMENT-TRAP`) and cross-dialect destructive DDL extraction.

use std::collections::BTreeSet;

use gf_core::{DestructiveDdlFinding, LangRuleFinding, MigrationViolation, Span, SqlDialectKind};
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, ColumnOption, ObjectType, Statement,
};
use sqlparser::tokenizer::Token;

use super::pg_types_defaults::normalize_table_key;

fn column_has_autoincrement(col: &ColumnDef) -> bool {
    col.options.iter().any(|opt| match &opt.option {
        ColumnOption::DialectSpecific(tokens) => tokens.iter().any(|tok| match tok {
            Token::Word(w) => w.value.eq_ignore_ascii_case("AUTOINCREMENT"),
            _ => false,
        }),
        _ => false,
    })
}

fn check_d1_transaction_stmt(stmt: &Statement, span: Span, out: &mut Vec<LangRuleFinding>) {
    let forbidden_tx = match stmt {
        Statement::StartTransaction { .. } => Some("BEGIN TRANSACTION"),
        Statement::Commit { .. } => Some("COMMIT"),
        Statement::Rollback { .. } => Some("ROLLBACK"),
        Statement::Savepoint { .. } | Statement::ReleaseSavepoint { .. } => Some("SAVEPOINT"),
        _ => None,
    };
    if let Some(kw) = forbidden_tx {
        out.push(LangRuleFinding::new(
            MigrationViolation::D1ForbidTransaction,
            span,
            format!(
                "explicit `{kw}` is rejected by Cloudflare D1 migrations (D1 manages migration transactions server-side)"
            ),
        ));
    }
}

fn emit_autoincrement_if_present(col: &ColumnDef, span: Span, out: &mut Vec<LangRuleFinding>) {
    if column_has_autoincrement(col) {
        out.push(LangRuleFinding::new(
            MigrationViolation::SqliteAutoincrementTrap,
            span,
            format!(
                "column `{}` uses `INTEGER PRIMARY KEY AUTOINCREMENT`; omit `AUTOINCREMENT` in SQLite/D1",
                col.name.value
            ),
        ));
    }
}

fn check_sqlite_autoincrement_stmt(stmt: &Statement, span: Span, out: &mut Vec<LangRuleFinding>) {
    match stmt {
        Statement::CreateTable(ct) => {
            for col in &ct.columns {
                emit_autoincrement_if_present(col, span, out);
            }
        }
        Statement::AlterTable(alter) => {
            for op in &alter.operations {
                if let AlterTableOperation::AddColumn { column_def, .. } = op {
                    emit_autoincrement_if_present(column_def, span, out);
                }
            }
        }
        _ => {}
    }
}

/// Evaluates `MIG-D1-FORBID-TRANSACTION` and `MIG-SQLITE-AUTOINCREMENT-TRAP` on `stmt`.
pub fn check_sqlite_and_d1_statement(
    stmt: &Statement,
    span: Span,
    dialect: SqlDialectKind,
    out: &mut Vec<LangRuleFinding>,
) {
    if dialect == SqlDialectKind::CloudflareD1 {
        check_d1_transaction_stmt(stmt, span, out);
    }
    if matches!(
        dialect,
        SqlDialectKind::Sqlite | SqlDialectKind::CloudflareD1
    ) {
        check_sqlite_autoincrement_stmt(stmt, span, out);
    }
}

fn collect_alter_destructive_ops(
    alter: &AlterTable,
    span: Span,
    out: &mut Vec<DestructiveDdlFinding>,
) {
    for op in &alter.operations {
        let detail = match op {
            AlterTableOperation::DropColumn { column_names, .. } => Some(format!(
                "destructive `ALTER TABLE {} DROP COLUMN {}` in up-migration",
                alter.name,
                column_names.first().map_or("...", |c| c.value.as_str())
            )),
            AlterTableOperation::RenameColumn {
                old_column_name,
                new_column_name,
            } => Some(format!(
                "destructive `ALTER TABLE {} RENAME COLUMN {old_column_name} TO {new_column_name}` in up-migration",
                alter.name
            )),
            AlterTableOperation::RenameTable { .. } => Some(format!(
                "destructive `ALTER TABLE {} RENAME` in up-migration",
                alter.name
            )),
            _ => None,
        };
        if let Some(detail) = detail {
            out.push(DestructiveDdlFinding { span, detail });
        }
    }
}

/// Tracks newly created tables across any SQL dialect and extracts destructive DDL on existing tables.
#[derive(Default)]
pub struct DestructiveDdlTracker {
    created_tables: BTreeSet<String>,
    in_tx: bool,
    tx_created_tables: BTreeSet<String>,
}

impl DestructiveDdlTracker {
    fn observe_tx_or_create(&mut self, stmt: &Statement) -> bool {
        match stmt {
            Statement::StartTransaction { .. } => {
                self.in_tx = true;
                self.tx_created_tables.clear();
                true
            }
            Statement::Commit { .. } => {
                self.in_tx = false;
                self.tx_created_tables.clear();
                true
            }
            Statement::Rollback {
                savepoint: None, ..
            } => {
                for t in &self.tx_created_tables {
                    self.created_tables.remove(t);
                }
                self.in_tx = false;
                self.tx_created_tables.clear();
                true
            }
            Statement::CreateTable(ct) => {
                let tbl = normalize_table_key(&ct.name);
                self.created_tables.insert(tbl.clone());
                if self.in_tx {
                    self.tx_created_tables.insert(tbl);
                }
                true
            }
            _ => false,
        }
    }

    /// Observes `stmt` in an up-migration and appends any destructive DDL on pre-existing tables.
    pub fn step(&mut self, stmt: &Statement, span: Span, out: &mut Vec<DestructiveDdlFinding>) {
        if self.observe_tx_or_create(stmt) {
            return;
        }
        match stmt {
            Statement::Drop {
                object_type: ObjectType::Table,
                names,
                ..
            } => {
                for name in names {
                    if !self.created_tables.contains(&normalize_table_key(name)) {
                        out.push(DestructiveDdlFinding {
                            span,
                            detail: format!("destructive `DROP TABLE {name}` in up-migration"),
                        });
                    }
                }
            }
            Statement::Truncate(tr) => out.push(DestructiveDdlFinding {
                span,
                detail: format!("destructive `TRUNCATE` (`{tr}`) in up-migration"),
            }),
            Statement::RenameTable(_) => out.push(DestructiveDdlFinding {
                span,
                detail: String::from("destructive `RENAME TABLE` in up-migration"),
            }),
            Statement::AlterTable(alter)
                if !self
                    .created_tables
                    .contains(&normalize_table_key(&alter.name)) =>
            {
                collect_alter_destructive_ops(alter, span, out);
            }
            _ => {}
        }
    }
}
