//! Pure `PostgreSQL 11+` non-volatile `DEFAULT` and `ADD COLUMN` / `IS NOT NULL` helpers.

use gf_core::{LangRuleFinding, MigrationViolation, Span};
use sqlparser::ast::{
    ColumnDef, ColumnOption, Expr, FunctionArg, FunctionArgExpr, FunctionArguments,
    GeneratedExpressionMode, Value,
};

pub use super::pg_types::{
    check_pg_banned_data_type, extract_varchar_or_text_width, is_pg_metadata_only_type_change,
    is_pg_serial_type, normalize_ident_key, normalize_table_key,
};

const PG_VOLATILE_FUNCTIONS: &[&str] = &[
    "clock_timestamp",
    "random",
    "gen_random_uuid",
    "uuid_generate_v1",
    "uuid_generate_v1mc",
    "uuid_generate_v4",
    "timeofday",
    "nextval",
    "setval",
    "currval",
    "lastval",
    "txid_current",
    "pg_backend_pid",
];

fn fn_args_are_non_volatile(args: &FunctionArguments) -> bool {
    match args {
        FunctionArguments::None => true,
        FunctionArguments::Subquery(_) => false,
        FunctionArguments::List(list) => list.args.iter().all(|arg| match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(e))
            | FunctionArg::Named {
                arg: FunctionArgExpr::Expr(e),
                ..
            }
            | FunctionArg::ExprNamed {
                arg: FunctionArgExpr::Expr(e),
                ..
            } => is_pg11_non_volatile_default(e),
            _ => false,
        }),
    }
}

fn is_pg11_non_volatile_default(expr: &Expr) -> bool {
    match expr {
        Expr::Value(v) => matches!(
            v.value,
            Value::Number(_, _)
                | Value::SingleQuotedString(_)
                | Value::DollarQuotedString(_)
                | Value::Boolean(_)
                | Value::Null
        ),
        Expr::Nested(inner)
        | Expr::UnaryOp { expr: inner, .. }
        | Expr::Cast { expr: inner, .. } => is_pg11_non_volatile_default(inner),
        Expr::BinaryOp { left, right, .. } => {
            is_pg11_non_volatile_default(left) && is_pg11_non_volatile_default(right)
        }
        Expr::TypedString { .. } => true,
        Expr::Function(func) => {
            let fn_name = normalize_table_key(&func.name);
            !PG_VOLATILE_FUNCTIONS.contains(&fn_name.as_str())
                && func.over.is_none()
                && fn_args_are_non_volatile(&func.args)
        }
        _ => false,
    }
}

fn has_stored_generated_option(col: &ColumnDef) -> bool {
    col.options.iter().any(|opt_def| {
        matches!(
            &opt_def.option,
            ColumnOption::Generated {
                generation_expr: Some(_),
                generation_expr_mode: Some(GeneratedExpressionMode::Stored) | None,
                ..
            }
        )
    })
}

fn add_column_unsafe_reason(table: &str, col: &ColumnDef) -> Option<String> {
    let col_name = col.name.value.as_str();
    if is_pg_serial_type(&col.data_type) {
        return Some(format!(
            "`ADD COLUMN {col_name}` with `{}` on existing table `{table}` rewrites the table via `nextval()`",
            col.data_type
        ));
    }
    if has_stored_generated_option(col) {
        return Some(format!(
            "`ADD COLUMN {col_name} GENERATED ... STORED` on existing table `{table}` rewrites all rows"
        ));
    }
    let has_not_null = col
        .options
        .iter()
        .any(|o| matches!(o.option, ColumnOption::NotNull));
    let default_expr = col.options.iter().find_map(|o| match &o.option {
        ColumnOption::Default(expr) => Some(expr),
        _ => None,
    });
    match (has_not_null, default_expr) {
        (true, None) => Some(format!(
            "`ADD COLUMN {col_name} NOT NULL` without a non-volatile `DEFAULT` locks and scans `{table}`"
        )),
        (_, Some(def)) if !is_pg11_non_volatile_default(def) => Some(format!(
            "`ADD COLUMN {col_name}` with volatile `DEFAULT {def}` rewrites existing table `{table}` under `ACCESS EXCLUSIVE`"
        )),
        _ => None,
    }
}

/// Inspects `ADD COLUMN` on an existing `PostgreSQL` table for `NOT NULL` / volatile `DEFAULT` rewrites.
pub(super) fn check_pg_add_column_safety(
    table: &str,
    col: &ColumnDef,
    span: Span,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(msg) = add_column_unsafe_reason(table, col) {
        out.push(LangRuleFinding::new(
            MigrationViolation::PgAddNotNullUnsafe,
            span,
            msg,
        ));
    }
}

/// Extracts `Some(col_key)` if `expr` is `col IS NOT NULL` (unwrapping parens).
#[must_use]
pub fn extract_is_not_null_column(mut expr: &Expr) -> Option<String> {
    while let Expr::Nested(inner) = expr {
        expr = inner;
    }
    match expr {
        Expr::IsNotNull(inner) => match inner.as_ref() {
            Expr::Identifier(ident) => Some(normalize_ident_key(&ident.value)),
            _ => None,
        },
        _ => None,
    }
}
