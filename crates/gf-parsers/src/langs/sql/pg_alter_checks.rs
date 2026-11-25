//! `PostgreSQL` `ALTER TABLE` zero-downtime constraint, `SET NOT NULL`, and `ALTER COLUMN TYPE`
//! evaluators (`squawk`-grade `GF080`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{LangRuleFinding, MigrationViolation, Span};
use sqlparser::ast::{AlterTable, AlterTableOperation, CheckConstraint, DataType, TableConstraint};

use super::pg_types::{PgAlterColInput, evaluate_pg_alter_column};
use super::pg_types_defaults::{
    check_pg_add_column_safety, check_pg_banned_data_type, extract_is_not_null_column,
    normalize_ident_key, normalize_table_key,
};

#[derive(Clone, Copy)]
struct AlterTableCtx<'a> {
    table: &'a str,
    is_new: bool,
    span: Span,
}

#[derive(Clone, Copy)]
struct ConstraintSpec<'a> {
    constraint: &'a TableConstraint,
    not_valid: bool,
}

#[derive(Clone, Copy)]
struct CheckSpec<'a> {
    chk: &'a CheckConstraint,
    not_valid: bool,
}

/// Mutable catalog state shared across sequential statements in a `PostgreSQL` migration.
#[derive(Default)]
pub(super) struct PgCatalogState {
    pub in_explicit_tx: bool,
    pub created_tables: BTreeSet<String>,
    pub tx_created_tables: BTreeSet<String>,
    pub col_types: BTreeMap<(String, String), DataType>,
    pub not_valid_in_tx: BTreeSet<(String, String)>,
    pub not_null_checks: BTreeMap<(String, String), String>,
    pub validated_not_null_cols: BTreeSet<(String, String)>,
}

impl PgCatalogState {
    fn check_pg_check(
        &mut self,
        ctx: AlterTableCtx<'_>,
        spec: CheckSpec<'_>,
        out: &mut Vec<LangRuleFinding>,
    ) {
        if !spec.not_valid {
            out.push(LangRuleFinding::new(
                MigrationViolation::PgConstraintNotValid,
                ctx.span,
                format!(
                    "`ADD CONSTRAINT ... CHECK` on existing table `{}` without `NOT VALID` scans `{}` under `ACCESS EXCLUSIVE`",
                    ctx.table, ctx.table
                ),
            ));
        } else if let Some(name) = &spec.chk.name {
            let cname = normalize_ident_key(&name.value);
            if self.in_explicit_tx {
                self.not_valid_in_tx
                    .insert((String::from(ctx.table), cname.clone()));
            }
            if let Some(col_key) = extract_is_not_null_column(&spec.chk.expr) {
                self.not_null_checks
                    .insert((String::from(ctx.table), cname), col_key);
            }
        }
    }

    fn check_add_constraint(
        &mut self,
        ctx: AlterTableCtx<'_>,
        spec: ConstraintSpec<'_>,
        out: &mut Vec<LangRuleFinding>,
    ) {
        match spec.constraint {
            TableConstraint::ForeignKey(fk) => {
                if !spec.not_valid {
                    out.push(LangRuleFinding::new(
                        MigrationViolation::PgConstraintNotValid,
                        ctx.span,
                        format!(
                            "`ADD CONSTRAINT ... FOREIGN KEY` on existing table `{}` without `NOT VALID` blocks writes during validation",
                            ctx.table
                        ),
                    ));
                } else if let Some(name) = &fk.name
                    && self.in_explicit_tx
                {
                    self.not_valid_in_tx
                        .insert((String::from(ctx.table), normalize_ident_key(&name.value)));
                }
            }
            TableConstraint::Check(chk) => {
                let cspec = CheckSpec {
                    chk,
                    not_valid: spec.not_valid,
                };
                self.check_pg_check(ctx, cspec, out);
            }
            TableConstraint::Unique(_) => {
                out.push(LangRuleFinding::new(
                    MigrationViolation::PgConstraintNotValid,
                    ctx.span,
                    format!(
                        "`ADD CONSTRAINT ... UNIQUE` on existing table `{}` locks writes; create a `UNIQUE INDEX CONCURRENTLY` then `ADD CONSTRAINT ... UNIQUE USING INDEX`",
                        ctx.table
                    ),
                ));
            }
            _ => {}
        }
    }

    fn check_validate_constraint(
        &mut self,
        ctx: AlterTableCtx<'_>,
        raw_name: &str,
        out: &mut Vec<LangRuleFinding>,
    ) {
        let key = (String::from(ctx.table), normalize_ident_key(raw_name));
        if self.not_valid_in_tx.contains(&key) {
            out.push(LangRuleFinding::new(
                MigrationViolation::PgConstraintNotValid,
                ctx.span,
                format!(
                    "`VALIDATE CONSTRAINT {raw_name}` in the same transaction as `ADD CONSTRAINT ... NOT VALID` holds locks across the full scan"
                ),
            ));
        } else if let Some(col_key) = self.not_null_checks.get(&key) {
            self.validated_not_null_cols
                .insert((String::from(ctx.table), col_key.clone()));
        }
    }

    fn handle_add_column(
        &mut self,
        ctx: AlterTableCtx<'_>,
        column_def: &sqlparser::ast::ColumnDef,
        out: &mut Vec<LangRuleFinding>,
    ) {
        check_pg_banned_data_type(&column_def.name.value, &column_def.data_type, ctx.span, out);
        if !ctx.is_new {
            check_pg_add_column_safety(ctx.table, column_def, ctx.span, out);
        }
        let col_key = normalize_ident_key(&column_def.name.value);
        self.col_types.insert(
            (String::from(ctx.table), col_key),
            column_def.data_type.clone(),
        );
    }

    fn handle_alter_table_op(
        &mut self,
        ctx: AlterTableCtx<'_>,
        op: &AlterTableOperation,
        out: &mut Vec<LangRuleFinding>,
    ) {
        match op {
            AlterTableOperation::AddColumn { column_def, .. } => {
                self.handle_add_column(ctx, column_def, out);
            }
            AlterTableOperation::AlterColumn {
                column_name,
                op: col_op,
            } => {
                let inp = PgAlterColInput {
                    table: ctx.table,
                    is_new: ctx.is_new,
                    column_name: &column_name.value,
                    op: col_op,
                    span: ctx.span,
                };
                evaluate_pg_alter_column(
                    &inp,
                    &self.validated_not_null_cols,
                    &mut self.col_types,
                    out,
                );
            }
            AlterTableOperation::AddConstraint {
                constraint,
                not_valid,
            } if !ctx.is_new => {
                let spec = ConstraintSpec {
                    constraint,
                    not_valid: *not_valid,
                };
                self.check_add_constraint(ctx, spec, out);
            }
            AlterTableOperation::ValidateConstraint { name } => {
                self.check_validate_constraint(ctx, &name.value, out);
            }
            _ => {}
        }
    }

    /// Evaluates `ALTER TABLE` operations for `PostgreSQL` zero-downtime safety.
    pub(super) fn evaluate_alter_table(
        &mut self,
        alter: &AlterTable,
        span: Span,
        out: &mut Vec<LangRuleFinding>,
    ) {
        let table = normalize_table_key(&alter.name);
        let ctx = AlterTableCtx {
            table: &table,
            is_new: self.created_tables.contains(&table),
            span,
        };
        for op in &alter.operations {
            self.handle_alter_table_op(ctx, op, out);
        }
    }
}
