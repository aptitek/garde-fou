//! Pure `PostgreSQL` identifier normalization, banned column types (`MIG-PG-BAN-TYPES`),
//! and safe `VARCHAR(n)` -> `VARCHAR(m)` / `TEXT` widening predicates (`MIG-PG-ALTER-COLUMN-TYPE`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{LangRuleFinding, MigrationViolation, Span};
use sqlparser::ast::{AlterColumnOperation, CharacterLength, DataType, ObjectName, TimezoneInfo};

const PG_SERIAL_TYPES: &[&str] = &[
    "serial",
    "bigserial",
    "smallserial",
    "serial2",
    "serial4",
    "serial8",
];

/// Normalizes a table or object name to its unqualified lowercase identifier (`public.users` -> `users`).
#[must_use]
pub fn normalize_table_key(name: &ObjectName) -> String {
    name.0.last().map_or_else(
        || name.to_string().to_ascii_lowercase(),
        |p| {
            p.to_string()
                .trim_matches('"')
                .trim_matches('`')
                .to_ascii_lowercase()
        },
    )
}

/// Normalizes a SQL identifier to lowercase without quotes.
#[must_use]
pub fn normalize_ident_key(raw: &str) -> String {
    raw.trim_matches('"').trim_matches('`').to_ascii_lowercase()
}

/// Checks if `data_type` is a `PostgreSQL` serial pseudo-type (`SERIAL`, `BIGSERIAL`, `SMALLSERIAL`).
#[must_use]
pub fn is_pg_serial_type(data_type: &DataType) -> bool {
    match data_type {
        DataType::Custom(name, _) => {
            let n = normalize_table_key(name);
            PG_SERIAL_TYPES.contains(&n.as_str())
        }
        _ => false,
    }
}

/// Checks `MIG-PG-BAN-TYPES` (`CHAR(n)` / `CHARACTER(n)` / `BPCHAR` and `TIMESTAMP [WITHOUT TIME ZONE]`).
pub fn check_pg_banned_data_type(
    col_name: &str,
    data_type: &DataType,
    span: Span,
    out: &mut Vec<LangRuleFinding>,
) {
    let banned_reason = match data_type {
        DataType::Char(_) | DataType::Character(_) => Some(
            "fixed-width `CHAR`/`CHARACTER` pads values and wastes space; use `TEXT` or `VARCHAR`",
        ),
        DataType::Custom(name, _)
            if matches!(normalize_table_key(name).as_str(), "bpchar" | "nchar") =>
        {
            Some(
                "`BPCHAR`/`NCHAR` fixed-width character type is discouraged; use `TEXT` or `VARCHAR`",
            )
        }
        DataType::Timestamp(_, TimezoneInfo::None | TimezoneInfo::WithoutTimeZone)
        | DataType::TimestampNtz(_) => Some(
            "`TIMESTAMP` without time zone loses UTC context; use `TIMESTAMPTZ` (`TIMESTAMP WITH TIME ZONE`)",
        ),
        _ => None,
    };
    if let Some(reason) = banned_reason {
        out.push(LangRuleFinding::new(
            MigrationViolation::PgBanTypes,
            span,
            format!("column `{col_name}`: {reason}"),
        ));
    }
}

/// Extracts `Some(Some(n))` for `VARCHAR(n)` or `Some(None)` for unconstrained `VARCHAR` / `TEXT`.
#[must_use]
pub const fn extract_varchar_or_text_width(dt: &DataType) -> Option<Option<u64>> {
    match dt {
        DataType::Text => Some(None),
        DataType::Varchar(len)
        | DataType::CharacterVarying(len)
        | DataType::CharVarying(len)
        | DataType::Nvarchar(len) => match len {
            None | Some(CharacterLength::Max) => Some(None),
            Some(CharacterLength::IntegerLength { length, .. }) => Some(Some(*length)),
        },
        _ => None,
    }
}

/// Returns `true` when changing a column type to `new_type` (given optional `prior_type`) is a
/// metadata-only widening in `PostgreSQL` (`VARCHAR(n) -> VARCHAR(m)` with `m >= n`, or `-> TEXT`).
#[must_use]
pub const fn is_pg_metadata_only_type_change(
    prior_type: Option<&DataType>,
    new_type: &DataType,
    has_using: bool,
) -> bool {
    if has_using {
        return false;
    }
    let Some(target_width) = extract_varchar_or_text_width(new_type) else {
        return false;
    };
    let Some(prev) = prior_type else {
        return true;
    };
    let Some(prev_width) = extract_varchar_or_text_width(prev) else {
        return false;
    };
    match (prev_width, target_width) {
        (_, None) => true,
        (Some(n), Some(m)) => m >= n,
        (None, Some(_)) => false,
    }
}

/// Parameters for evaluating `ALTER TABLE ... ALTER COLUMN` on `PostgreSQL`.
pub struct PgAlterColInput<'a> {
    /// Normalized lowercase table name.
    pub table: &'a str,
    /// Whether the table was created earlier in the current migration.
    pub is_new: bool,
    /// Column name as written in SQL.
    pub column_name: &'a str,
    /// Parsed `ALTER COLUMN` operation.
    pub op: &'a AlterColumnOperation,
    /// 1-based statement span.
    pub span: Span,
}

/// Evaluates `ALTER COLUMN` (`SET NOT NULL` and `SET DATA TYPE`) against tracked catalog state.
pub fn evaluate_pg_alter_column(
    inp: &PgAlterColInput<'_>,
    validated_not_null_cols: &BTreeSet<(String, String)>,
    col_types: &mut BTreeMap<(String, String), DataType>,
    out: &mut Vec<LangRuleFinding>,
) {
    let key = (
        String::from(inp.table),
        normalize_ident_key(inp.column_name),
    );
    match inp.op {
        AlterColumnOperation::SetNotNull
            if !inp.is_new && !validated_not_null_cols.contains(&key) =>
        {
            out.push(LangRuleFinding::new(
                MigrationViolation::PgAddNotNullUnsafe,
                inp.span,
                format!(
                    "`ALTER COLUMN {} SET NOT NULL` on existing table `{}` without a validated `CHECK (... IS NOT NULL)` constraint blocks writes",
                    inp.column_name, inp.table
                ),
            ));
        }
        AlterColumnOperation::SetDataType {
            data_type, using, ..
        } => {
            check_pg_banned_data_type(inp.column_name, data_type, inp.span, out);
            let prior = col_types.get(&key);
            if !inp.is_new && !is_pg_metadata_only_type_change(prior, data_type, using.is_some()) {
                out.push(LangRuleFinding::new(
                    MigrationViolation::PgAlterColumnType,
                    inp.span,
                    format!(
                        "`ALTER COLUMN {} TYPE {data_type}` rewrites existing table `{}` under `ACCESS EXCLUSIVE`",
                        inp.column_name, inp.table
                    ),
                ));
            }
            col_types.insert(key, data_type.clone());
        }
        _ => {}
    }
}
