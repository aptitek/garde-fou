//! Tier 2 static configuration hardening (`GF081`) for `.squawk.toml` / `squawk.toml`,
//! `.sqruff`, `.sqlfluff`, and `drizzle.config.*`.

use gf_core::{
    RepoPath, Span, SqlToolConfigIssue, SqruffSqlfluffConfigIssue, SquawkConfigIssue,
    SyntaxErrorNode, ToolConfigFinding, ToolConfigViolation, UastDocument,
};
use toml_edit::{ImDocument, Item};

use super::drizzle_config::audit_drizzle_config;
use crate::langs::toml_lang::span_from_byte_offset;

const CRITICAL_SQUAWK_RULES: &[&str] = &[
    "require-concurrent-index-creation",
    "require-concurrent-index-deletion",
    "adding-not-null-field",
    "adding-field-with-default",
    "constraint-missing-not-valid",
    "disallow-unique-constraint",
    "changing-column-type",
    "ban-drop-table",
    "ban-drop-column",
];

const BLANKET_SQL_GLOBS: &[&str] = &[
    "*",
    "**",
    "**/*",
    "**/*.sql",
    "*.sql",
    "migrations",
    "migrations/*",
    "migrations/**",
];

const CRITICAL_SQRUFF_SQLFLUFF_RULES: &[&str] = &["al09", "cv05", "l049", "am04"];

fn squawk_array<'a>(
    source: &str,
    tbl: &'a toml_edit::Table,
    key: &str,
) -> Option<(Span, &'a toml_edit::Array)> {
    let item @ Item::Value(val) = tbl.get(key)? else {
        return None;
    };
    let arr = val.as_array()?;
    let span = tbl
        .key(key)
        .and_then(toml_edit::Key::span)
        .or_else(|| item.span())
        .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
    Some((span, arr))
}

fn push_sql_cfg_finding(
    out: &mut Vec<ToolConfigFinding>,
    issue: SqlToolConfigIssue,
    span: Span,
    message: String,
) {
    out.push(ToolConfigFinding {
        violation: ToolConfigViolation::Sql(issue),
        span,
        message,
    });
}

fn check_squawk_excluded_rules(
    span: Span,
    arr: &toml_edit::Array,
    out: &mut Vec<ToolConfigFinding>,
) {
    for s in arr.iter().filter_map(toml_edit::Value::as_str) {
        if CRITICAL_SQUAWK_RULES.contains(&s.trim().to_ascii_lowercase().as_str()) {
            push_sql_cfg_finding(
                out,
                SqlToolConfigIssue::Squawk(SquawkConfigIssue::CriticalRuleExcluded),
                span,
                format!(
                    "`.squawk.toml` disables critical zero-downtime migration rule `{s}` in `excluded_rules`"
                ),
            );
        }
    }
}

fn check_squawk_excluded_paths(
    span: Span,
    arr: &toml_edit::Array,
    out: &mut Vec<ToolConfigFinding>,
) {
    for s in arr.iter().filter_map(toml_edit::Value::as_str) {
        if BLANKET_SQL_GLOBS.contains(&s.trim().trim_start_matches("./")) {
            push_sql_cfg_finding(
                out,
                SqlToolConfigIssue::Squawk(SquawkConfigIssue::BlanketExcludedPaths),
                span,
                format!("`.squawk.toml` blanket-excludes `{s}` in `excluded_paths`"),
            );
        }
    }
}

fn audit_squawk_toml(source: &str, doc: &mut UastDocument) {
    let parsed = match ImDocument::parse(source) {
        Ok(d) => d,
        Err(err) => {
            let span = err
                .span()
                .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
            doc.syntax_errors.push(SyntaxErrorNode {
                span,
                message: err.message().to_string(),
            });
            return;
        }
    };
    let tbl = parsed.as_table();
    if let Some((span, arr)) = squawk_array(source, tbl, "excluded_rules") {
        check_squawk_excluded_rules(span, arr, &mut doc.tool_config_findings);
    }
    if let Some((span, arr)) = squawk_array(source, tbl, "excluded_paths") {
        check_squawk_excluded_paths(span, arr, &mut doc.tool_config_findings);
    }
}

fn classify_ini_rule_token(item: &str) -> Option<SqruffSqlfluffConfigIssue> {
    let low = item.to_ascii_lowercase();
    if low == "all" || low == "*" {
        Some(SqruffSqlfluffConfigIssue::ExcludeAllRules)
    } else if CRITICAL_SQRUFF_SQLFLUFF_RULES.contains(&low.as_str()) {
        Some(SqruffSqlfluffConfigIssue::CriticalRuleExcluded)
    } else {
        None
    }
}

fn audit_sqruff_or_sqlfluff_ini(is_sqruff: bool, source: &str, out: &mut Vec<ToolConfigFinding>) {
    let label = if is_sqruff { ".sqruff" } else { ".sqlfluff" };
    for (idx, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.split('#').next().unwrap_or("").trim();
        let Some((key, val)) = trimmed.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("exclude_rules") {
            continue;
        }
        let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let span = Span::new(ln, ln).unwrap_or(Span::FIRST);
        for item in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let Some(kind) = classify_ini_rule_token(item) else {
                continue;
            };
            let issue = if is_sqruff {
                SqlToolConfigIssue::Sqruff(kind)
            } else {
                SqlToolConfigIssue::Sqlfluff(kind)
            };
            push_sql_cfg_finding(
                out,
                issue,
                span,
                format!("`{label}` disables `{item}` in `exclude_rules`"),
            );
        }
    }
}

/// Audits a SQL/migration configuration file (`.squawk.toml`, `.sqruff`, `.sqlfluff`, `drizzle.config.*`).
pub fn audit_sql_tool_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    match path.file_name() {
        ".squawk.toml" | "squawk.toml" => audit_squawk_toml(source, doc),
        ".sqruff" => audit_sqruff_or_sqlfluff_ini(true, source, &mut doc.tool_config_findings),
        ".sqlfluff" => audit_sqruff_or_sqlfluff_ini(false, source, &mut doc.tool_config_findings),
        "drizzle.config.ts" | "drizzle.config.js" | "drizzle.config.mjs" => {
            audit_drizzle_config(source, doc);
        }
        _ => {}
    }
}
