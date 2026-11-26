//! Contextual SQL dialect resolution, migration role/version classification,
//! `GF070` inline suppression auditing, and `sqlparser` span helpers.

use gf_core::{BypassFinding, RepoPath, Span, SqlDialectKind, SqlRole};
use sqlparser::dialect::{
    Dialect, DuckDbDialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect,
};

const MIGRATION_DIR_PREFIXES: &[&str] = &[
    "migrations/",
    "drizzle/",
    "prisma/migrations/",
    "db/migrate/",
];

/// Converts a 1-based `u64` line from `sqlparser` (`0` when absent) into a [`Span`].
#[must_use]
pub fn span_from_sql_line(line: u64) -> Span {
    let l = u32::try_from(line).unwrap_or(u32::MAX).max(1);
    Span::new(l, l).unwrap_or(Span::FIRST)
}

/// Converts a `(start_line, end_line)` pair from `sqlparser` into a [`Span`].
#[must_use]
pub fn span_from_sql_range(start_line: u64, end_line: u64) -> Span {
    let s = u32::try_from(start_line).unwrap_or(u32::MAX).max(1);
    let e = u32::try_from(end_line).unwrap_or(u32::MAX).max(s);
    Span::new(s, e).unwrap_or(Span::FIRST)
}

/// Extracts exact 1-based `(line, col)` from a `sqlparser` error string (`" at Line: L, Column: C"`).
#[must_use]
pub fn extract_sqlparser_error_loc(msg: &str) -> (u32, u32) {
    let Some((_, after)) = msg.rsplit_once("Line:") else {
        return (1, 1);
    };
    let mut parts = after.split(',');
    let line = parts
        .next()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let col = parts
        .next()
        .and_then(|s| s.trim().strip_prefix("Column:"))
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    (line, col)
}

/// Instantiates the `sqlparser` [`Dialect`] implementation for `kind`.
#[must_use]
pub fn make_sqlparser_dialect(kind: SqlDialectKind) -> Box<dyn Dialect> {
    match kind {
        SqlDialectKind::Postgres => Box::new(PostgreSqlDialect {}),
        SqlDialectKind::Sqlite | SqlDialectKind::CloudflareD1 => Box::new(SQLiteDialect {}),
        SqlDialectKind::MySql | SqlDialectKind::MariaDb => Box::new(MySqlDialect {}),
        SqlDialectKind::DuckDb => Box::new(DuckDbDialect {}),
        SqlDialectKind::Generic | SqlDialectKind::OraclePlSql => Box::new(GenericDialect {}),
    }
}

fn parse_dialect_pragma(source: &str) -> Option<SqlDialectKind> {
    for line in source.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(comment) = trimmed.strip_prefix("--") else {
            if !trimmed.starts_with("/*") {
                break;
            }
            continue;
        };
        if let Some(rest) = comment.trim().strip_prefix("garde-dialect:") {
            let token = rest.split_whitespace().next().unwrap_or("");
            return SqlDialectKind::from_config_str(token);
        }
    }
    None
}

fn leading_digits(s: &str) -> Option<String> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}

fn strip_migration_suffix(file_name: &str) -> (&str, bool) {
    let lower = file_name.to_ascii_lowercase();
    if lower == "down.sql" {
        return ("migration", true);
    }
    if lower == "migration.sql" || lower == "up.sql" {
        return ("migration", false);
    }
    for (suffix, is_down) in [
        (".down.sql", true),
        ("_down.sql", true),
        (".up.sql", false),
        ("_up.sql", false),
        (".sql", false),
    ] {
        if lower.ends_with(suffix) {
            let cut = file_name.len() - suffix.len();
            return (&file_name[..cut], is_down);
        }
    }
    (file_name, false)
}

/// Classifies `path` into [`SqlRole`] and its migration grouping directory.
#[must_use]
pub fn classify_sql_role_and_group(path: &RepoPath) -> (SqlRole, String) {
    let raw = path.as_str();
    let file_name = path.file_name();
    let lower_name = file_name.to_ascii_lowercase();
    let in_mig_dir = MIGRATION_DIR_PREFIXES
        .iter()
        .any(|p| raw.starts_with(p) || raw.contains(&format!("/{p}")));
    let has_up_down_ext = lower_name.ends_with(".up.sql") || lower_name.ends_with(".down.sql");
    if !in_mig_dir && !has_up_down_ext {
        return (SqlRole::QueryOrSchema, String::new());
    }

    let (parent_dir, _) = raw.rsplit_once('/').unwrap_or((".", raw));
    // Prisma nests each migration in `prisma/migrations/<timestamp_name>/migration.sql`.
    if matches!(lower_name.as_str(), "migration.sql" | "down.sql")
        && let Some((grand_parent, folder_name)) = parent_dir.rsplit_once('/')
    {
        return (
            SqlRole::Migration {
                version_prefix: leading_digits(folder_name),
                migration_stem: String::from(folder_name),
                is_down: lower_name == "down.sql",
            },
            String::from(grand_parent),
        );
    }

    let (stem, is_down) = strip_migration_suffix(file_name);
    (
        SqlRole::Migration {
            version_prefix: leading_digits(stem),
            migration_stem: String::from(stem),
            is_down,
        },
        String::from(parent_dir),
    )
}

fn has_sqlite_table_signature(source: &str) -> bool {
    let upper = source.to_ascii_uppercase();
    upper.contains("WITHOUT ROWID")
        || upper.contains("AUTOINCREMENT")
        || upper.contains(") STRICT;")
        || upper.contains(") STRICT\n")
}

fn detect_mariadb_or_plsql_signature(source: &str) -> Option<SqlDialectKind> {
    let upper = source.to_ascii_uppercase();
    if [
        "DELIMITER ",
        "ENGINE=INNODB",
        "ENGINE=MYISAM",
        "ALGORITHM=INSTANT",
    ]
    .iter()
    .any(|kw| upper.contains(kw))
    {
        return Some(SqlDialectKind::MariaDb);
    }
    if [
        "AUTHID CURRENT_USER",
        "PRAGMA AUTONOMOUS_TRANSACTION",
        "CREATE OR REPLACE PACKAGE",
        "EXECUTE IMMEDIATE",
    ]
    .iter()
    .any(|kw| upper.contains(kw))
    {
        return Some(SqlDialectKind::OraclePlSql);
    }
    None
}

/// Resolves the effective [`SqlDialectKind`] for a `.sql` document.
#[must_use]
pub fn resolve_sql_dialect(source: &str, ws_hint: Option<SqlDialectKind>) -> SqlDialectKind {
    if let Some(pragma) = parse_dialect_pragma(source) {
        return pragma;
    }
    if let Some(hint) = ws_hint {
        return hint;
    }
    if let Some(sig) = detect_mariadb_or_plsql_signature(source) {
        return sig;
    }
    if has_sqlite_table_signature(source) {
        return SqlDialectKind::Sqlite;
    }
    SqlDialectKind::Postgres
}

fn check_sql_suppression_body(trimmed: &str) -> Option<&'static str> {
    let (directive, rest) = [
        ("squawk-ignore", "-- squawk-ignore"),
        ("noqa", "-- noqa"),
        ("sqlfluff:disable", "-- sqlfluff:disable"),
        ("sqruff:disable", "-- sqruff:disable"),
    ]
    .into_iter()
    .find_map(|(pfx, label)| trimmed.strip_prefix(pfx).map(|r| (label, r)))?;

    let body = rest.trim_start_matches([':', ' ', '\t']);
    let (rules_part, reason_part) = match body.split_once("--") {
        Some((r, why)) => (r.trim(), why.trim()),
        None => return Some(directive),
    };
    let lower_rules = rules_part.to_ascii_lowercase();
    let valid_rules = !rules_part.is_empty() && lower_rules != "all" && lower_rules != "*";
    if valid_rules && reason_part.len() >= 10 {
        None
    } else {
        Some(directive)
    }
}

/// Scans SQL line comments for unjustified `-- squawk-ignore` / `-- noqa` / `-- sqlfluff:disable`
/// / `-- sqruff:disable` suppressions (`GF070`).
pub fn scan_sql_bypass_comments(source: &str, out: &mut Vec<BypassFinding>) {
    for (idx, line) in source.lines().enumerate() {
        let Some((_, after_dashes)) = line.split_once("--") else {
            continue;
        };
        let trimmed = after_dashes.trim();
        if let Some(directive) = check_sql_suppression_body(trimmed) {
            let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            out.push(BypassFinding {
                directive: String::from(directive),
                span: Span::new(ln, ln).unwrap_or(Span::FIRST),
            });
        }
    }
}
