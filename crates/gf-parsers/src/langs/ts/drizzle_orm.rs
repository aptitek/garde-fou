//! Drizzle ORM dialect alignment, bounded `.where(...)` mutations, `sql.raw` safety,
//! foreign-key indexing, and monetary column types (`DRIZZLE-*` — `GF080`).

use crate::cst::{node_span, node_text};
use gf_core::{DrizzleViolation, LangRuleFinding, SqlDialectKind, UastDocument, WebEdgeSummary};
use std::collections::BTreeSet;
use tree_sitter::Node;

const TABLE_BUILDERS: &[&str] = &["sqliteTable", "pgTable", "mysqlTable"];
const MONETARY_STEMS: &[&str] = &["price", "amount", "balance", "total", "cost", "fee"];

fn is_monetary_column_name(name: &str) -> bool {
    let low = name.trim().to_ascii_lowercase();
    MONETARY_STEMS.iter().any(|&s| {
        low == s
            || low.ends_with(&format!("_{s}"))
            || name.ends_with(&format!("{}{}", s[..1].to_ascii_uppercase(), &s[1..]))
    })
}

fn is_float_column_builder(expr_text: &str) -> bool {
    ["real(", "doublePrecision(", "float("]
        .iter()
        .any(|fn_pfx| expr_text.contains(fn_pfx))
}

#[rustfmt::skip]
fn infer_local_drizzle_dialect(source: &str, doc: &UastDocument, ws_hint: Option<SqlDialectKind>) -> Option<SqlDialectKind> {
    if let Some(h) = ws_hint {
        return Some(h);
    }
    for imp in &doc.imports {
        match imp.module_source.as_str() {
            "drizzle-orm/d1" | "drizzle-orm/libsql" | "drizzle-orm/better-sqlite3" => return Some(SqlDialectKind::CloudflareD1),
            "drizzle-orm/node-postgres" | "drizzle-orm/postgres-js" | "drizzle-orm/neon-http" => return Some(SqlDialectKind::Postgres),
            "drizzle-orm/mysql2" => return Some(SqlDialectKind::MySql),
            _ => {}
        }
    }
    (source.contains("D1Database") || source.contains("d1_databases")).then_some(SqlDialectKind::CloudflareD1)
}

fn core_import_dialect(module_source: &str) -> Option<SqlDialectKind> {
    match module_source {
        "drizzle-orm/sqlite-core" => Some(SqlDialectKind::Sqlite),
        "drizzle-orm/pg-core" => Some(SqlDialectKind::Postgres),
        "drizzle-orm/mysql-core" => Some(SqlDialectKind::MySql),
        _ => None,
    }
}

fn is_core_compatible_with_expected(expected: SqlDialectKind, imported: SqlDialectKind) -> bool {
    match expected {
        SqlDialectKind::Sqlite | SqlDialectKind::CloudflareD1 => imported == SqlDialectKind::Sqlite,
        other => other == imported,
    }
}

#[rustfmt::skip]
fn check_drizzle_core_imports(source: &str, ws_hint: Option<SqlDialectKind>, doc: &mut UastDocument) {
    let expected = infer_local_drizzle_dialect(source, doc, ws_hint);
    let we = doc.web_edge.get_or_insert_with(WebEdgeSummary::default);
    for imp in &doc.imports {
        let Some(imp_d) = core_import_dialect(imp.module_source.as_str()) else { continue };
        we.drizzle_dialect_imports.push((imp_d, imp.span));
        if let Some(exp) = expected
            && !is_core_compatible_with_expected(exp, imp_d)
        {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                DrizzleViolation::DialectMismatch,
                imp.span,
                format!("imported `{}` in a `{exp}` project; use matching Drizzle core", imp.module_source),
            ));
        }
    }
}

fn collect_indexed_columns(extra_node: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    if extra_node.kind() == "call_expression"
        && let Some(func) = extra_node.child_by_field_name("function")
        && func.kind() == "member_expression"
        && func
            .child_by_field_name("property")
            .is_some_and(|p| node_text(p, source).trim() == "on")
        && let Some(args) = extra_node.child_by_field_name("arguments")
    {
        collect_member_props(args, source, out);
    }
    let mut c = extra_node.walk();
    for ch in extra_node.children(&mut c) {
        collect_indexed_columns(ch, source, out);
    }
}

fn collect_member_props(node: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    if node.kind() == "member_expression"
        && let Some(prop) = node.child_by_field_name("property")
    {
        out.insert(String::from(node_text(prop, source).trim()));
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        collect_member_props(ch, source, out);
    }
}

fn inspect_table_column_pair(
    pair: Node<'_>,
    source: &str,
    indexed_cols: &BTreeSet<String>,
    out: &mut Vec<LangRuleFinding>,
) {
    let (Some(k_node), Some(v_node)) = (
        pair.child_by_field_name("key"),
        pair.child_by_field_name("value"),
    ) else {
        return;
    };
    let col_name = node_text(k_node, source).trim().trim_matches(['"', '\'']);
    let builder_txt = node_text(v_node, source);
    let span = node_span(pair);
    if is_monetary_column_name(col_name) && is_float_column_builder(builder_txt) {
        out.push(LangRuleFinding::new(
            DrizzleViolation::FloatMoney,
            span,
            format!("monetary column `{col_name}` uses floating-point type; use integer cents or `numeric`"),
        ));
    }
    if builder_txt.contains(".references(")
        && !builder_txt.contains(".primaryKey(")
        && !builder_txt.contains(".unique(")
        && !indexed_cols.contains(col_name)
    {
        out.push(LangRuleFinding::new(
            DrizzleViolation::FkMissingIndex,
            span,
            format!("foreign-key column `{col_name}` with `.references(...)` has no covering `index()` in table config"),
        ));
    }
}

fn inspect_drizzle_table_call(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(args) = node.child_by_field_name("arguments") else {
        return;
    };
    let Some(cols_obj) = args.named_child(1).filter(|n| n.kind() == "object") else {
        return;
    };
    let mut indexed_cols = BTreeSet::new();
    if let Some(extra_fn) = args.named_child(2) {
        collect_indexed_columns(extra_fn, source, &mut indexed_cols);
    }
    let mut c = cols_obj.walk();
    for ch in cols_obj.named_children(&mut c) {
        if ch.kind() == "pair" {
            inspect_table_column_pair(ch, source, &indexed_cols, out);
        }
    }
}

fn ancestor_chain_has_where(mut node: Node<'_>, source: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "member_expression"
            && let Some(prop) = parent.child_by_field_name("property")
        {
            if node_text(prop, source).trim() == "where" {
                return true;
            }
            node = parent;
        } else if parent.child_by_field_name("function") == Some(node) {
            node = parent;
        } else {
            break;
        }
    }
    false
}

#[rustfmt::skip]
fn is_unbounded_mutation_member(func: Node<'_>, callee: &str, source: &str) -> Option<&'static str> {
    if func.kind() != "member_expression" {
        return None;
    }
    let method = node_text(func.child_by_field_name("property")?, source).trim();
    if method == "set" && callee.contains(".update(") {
        return Some("set");
    }
    let is_db_delete = method == "delete"
        && func
            .child_by_field_name("object")
            .is_some_and(|o| matches!(node_text(o, source).trim(), "db" | "tx" | "trx" | "database"));
    is_db_delete.then_some("delete")
}

fn inspect_drizzle_call(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let callee = node_text(func, source).trim();
    if TABLE_BUILDERS.contains(&callee) {
        inspect_drizzle_table_call(node, source, out);
        return;
    }
    if callee == "sql.raw" {
        if let Some(arg0) = node
            .child_by_field_name("arguments")
            .and_then(|a| a.named_child(0))
            && (node_text(arg0, source).contains("${")
                || matches!(arg0.kind(), "binary_expression" | "identifier"))
        {
            out.push(LangRuleFinding::new(
                DrizzleViolation::RawUnsafe,
                node_span(node),
                String::from("`sql.raw(...)` with dynamic interpolation is vulnerable to SQL injection; use `` sql`...` ``"),
            ));
        }
        return;
    }
    if let Some(method) = is_unbounded_mutation_member(func, callee, source)
        && !ancestor_chain_has_where(node, source)
    {
        out.push(LangRuleFinding::new(
            DrizzleViolation::UnboundedMutation,
            node_span(node),
            format!("Drizzle `.{method}(...)` mutation executed without chaining `.where(...)`"),
        ));
    }
}

fn walk_drizzle_ast(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if node.kind() == "call_expression" {
        inspect_drizzle_call(node, source, out);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk_drizzle_ast(ch, source, out);
    }
}

/// Evaluates Drizzle ORM rules (`DRIZZLE-*`) on a TypeScript/JavaScript CST.
pub fn extract_drizzle_orm_rules(
    root: Node<'_>,
    source: &str,
    ws_hint: Option<SqlDialectKind>,
    doc: &mut UastDocument,
) {
    check_drizzle_core_imports(source, ws_hint, doc);
    walk_drizzle_ast(root, source, &mut doc.lang_rule_findings);
}
