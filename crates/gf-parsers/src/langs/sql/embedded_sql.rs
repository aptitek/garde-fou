//! Cross-language embedded SQL injection CST bridge (`SQL-INJECTION-INTERPOLATION` — `GF080`)
//! for TypeScript/JavaScript, Python, and Rust in `Code`.

use gf_core::{LangRuleFinding, SqlViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "INSERT", "UPDATE", "DELETE", "WITH", "CREATE", "ALTER", "DROP", "TRUNCATE", "MERGE",
];

const TS_SQL_METHODS: &[&str] = &[
    "query",
    "execute",
    "prepare",
    "exec",
    "raw",
    "$queryRawUnsafe",
    "$executeRawUnsafe",
];

const PY_SQL_METHODS: &[&str] = &[
    "execute",
    "executemany",
    "executescript",
    "raw",
    "text",
    "execute_sql",
];

const RUST_SQL_METHODS: &[&str] = &[
    "query",
    "query_as",
    "query_scalar",
    "raw_sql",
    "sql_query",
    "execute",
    "batch_execute",
    "prepare",
];

const TS_DYN_KINDS: &[&str] = &[
    "identifier",
    "member_expression",
    "call_expression",
    "template_substitution",
];

fn contains_sql_keyword(raw: &str) -> bool {
    raw.split(|c: char| !c.is_ascii_alphabetic())
        .any(|word| SQL_KEYWORDS.iter().any(|kw| word.eq_ignore_ascii_case(kw)))
}

fn has_descendant_kind(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|ch| has_descendant_kind(ch, kind))
}

fn ts_arg_is_interpolated_sql(arg: Node<'_>, source: &str, force_sql: bool) -> bool {
    let text = node_text(arg, source);
    let has_sql = force_sql || contains_sql_keyword(text);
    match arg.kind() {
        "parenthesized_expression" => arg
            .named_child(0)
            .is_some_and(|inner| ts_arg_is_interpolated_sql(inner, source, force_sql)),
        "template_string" => has_sql && has_descendant_kind(arg, "template_substitution"),
        "binary_expression" if text.contains('+') && has_sql => TS_DYN_KINDS
            .iter()
            .any(|kind| has_descendant_kind(arg, kind)),
        _ => false,
    }
}

/// Scans a TypeScript/JavaScript CST for dynamic string interpolation inside SQL execution sinks.
///
/// Tagged template literals (`` sql`...` ``, `` prisma.$queryRaw`...` ``) have `arguments.kind() == "template_string"`
/// rather than `"arguments"`, so they are structurally excluded with zero false positives.
pub fn extract_ts_embedded_sql(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    walk_ts_sql(root, source, out);
}

fn walk_ts_sql(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if node.kind() == "call_expression"
        && let Some(func) = node.child_by_field_name("function")
        && func.kind() == "member_expression"
        && let Some(prop) = func.child_by_field_name("property")
        && let Some(args) = node
            .child_by_field_name("arguments")
            .filter(|a| a.kind() == "arguments")
        && let Some(arg0) = args.named_child(0)
    {
        let method = node_text(prop, source).trim();
        let force_sql = matches!(method, "$queryRawUnsafe" | "$executeRawUnsafe");
        if TS_SQL_METHODS.contains(&method) && ts_arg_is_interpolated_sql(arg0, source, force_sql) {
            out.push(LangRuleFinding::new(
                SqlViolation::InjectionInterpolation,
                node_span(node),
                format!(
                    "interpolated SQL string passed to `.{method}(...)`; use parameterized placeholders or a safe tagged template"
                ),
            ));
        }
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_ts_sql(ch, source, out);
    }
}

fn py_callee_method<'a>(func: Node<'a>, source: &'a str) -> Option<&'a str> {
    match func.kind() {
        "attribute" => func
            .child_by_field_name("attribute")
            .map(|a| node_text(a, source).trim()),
        "identifier" => {
            let name = node_text(func, source).trim();
            (name == "text").then_some(name)
        }
        _ => None,
    }
}

fn py_arg_is_interpolated_sql(arg: Node<'_>, source: &str) -> bool {
    let text = node_text(arg, source);
    match arg.kind() {
        "parenthesized_expression" => arg
            .named_child(0)
            .is_some_and(|inner| py_arg_is_interpolated_sql(inner, source)),
        "string" | "concatenated_string" => {
            has_descendant_kind(arg, "interpolation") && contains_sql_keyword(text)
        }
        "call" => arg
            .child_by_field_name("function")
            .filter(|f| f.kind() == "attribute")
            .is_some_and(|f| {
                let obj = f.child_by_field_name("object");
                let attr = f.child_by_field_name("attribute");
                attr.is_some_and(|a| node_text(a, source).trim() == "format")
                    && obj.is_some_and(|o| contains_sql_keyword(node_text(o, source)))
            }),
        "binary_operator" => {
            let op_ok = text.contains('%') || text.contains('+');
            let left = arg.child_by_field_name("left");
            let right = arg.child_by_field_name("right");
            op_ok
                && left.is_some_and(|l| contains_sql_keyword(node_text(l, source)))
                && right.is_some_and(|r| r.kind() != "string")
        }
        _ => false,
    }
}

/// Scans a Python CST for `f"SELECT ... {x}"`, `"SELECT ...".format(...)`, or `"SELECT ... %s" % x`
/// passed to DB-API / ORM execution sinks.
pub fn extract_python_embedded_sql(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    walk_py_sql(root, source, out);
}

fn walk_py_sql(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if node.kind() == "call"
        && let Some(func) = node.child_by_field_name("function")
        && let Some(method) = py_callee_method(func, source)
        && PY_SQL_METHODS.contains(&method)
        && let Some(args) = node.child_by_field_name("arguments")
        && let Some(arg0) = args.named_child(0)
        && py_arg_is_interpolated_sql(arg0, source)
    {
        out.push(LangRuleFinding::new(
            SqlViolation::InjectionInterpolation,
            node_span(node),
            format!(
                "formatted/interpolated SQL string passed to `{method}(...)`; pass query parameters out-of-band"
            ),
        ));
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_py_sql(ch, source, out);
    }
}

fn rust_sql_callee_name<'a>(func: Node<'a>, source: &'a str) -> Option<&'a str> {
    match func.kind() {
        "scoped_identifier" => func
            .child_by_field_name("name")
            .map(|n| node_text(n, source).trim()),
        "field_expression" => func
            .child_by_field_name("field")
            .map(|f| node_text(f, source).trim()),
        _ => None,
    }
}

fn rust_arg_has_format_sql(arg: Node<'_>, source: &str) -> bool {
    let text = node_text(arg, source);
    if !contains_sql_keyword(text) {
        return false;
    }
    has_rust_format_macro(arg, source)
}

fn has_rust_format_macro(node: Node<'_>, source: &str) -> bool {
    if node.kind() == "macro_invocation"
        && let Some(m) = node.child_by_field_name("macro")
    {
        let m_name = node_text(m, source).trim();
        if m_name == "format" || m_name.ends_with("::format") {
            return true;
        }
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|ch| has_rust_format_macro(ch, source))
}

/// Checks a single Rust CST node (outside `#[cfg(test)]`) for `format!("SELECT ... {x}")` passed to `sqlx`/`diesel`/DB sinks.
pub fn inspect_rust_embedded_sql_node(
    node: Node<'_>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if node.kind() == "call_expression"
        && let Some(func) = node.child_by_field_name("function")
        && let Some(callee) = rust_sql_callee_name(func, source)
        && RUST_SQL_METHODS.contains(&callee)
        && let Some(args) = node.child_by_field_name("arguments")
        && let Some(arg0) = args.named_child(0)
        && rust_arg_has_format_sql(arg0, source)
    {
        out.push(LangRuleFinding::new(
            SqlViolation::InjectionInterpolation,
            node_span(node),
            format!(
                "`format!(...)` SQL string passed to `{callee}(...)`; use `$1`/`?` placeholders with `.bind(...)` or `sqlx::query!(...)`"
            ),
        ));
    }
}
