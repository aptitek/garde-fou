//! Tier 2 static configuration hardening (`GF081`) for `drizzle.config.{ts,js,mjs}`.

use gf_core::{
    DrizzleConfigIssue, SqlToolConfigIssue, ToolConfigFinding, ToolConfigViolation, UastDocument,
};
use tree_sitter::Node;

use crate::cst::{collect_syntax_errors, node_span, node_text, parse_with_language};

const DB_URL_SCHEMES: &[&str] = &[
    "postgres://",
    "postgresql://",
    "mysql://",
    "mariadb://",
    "libsql://",
    "sqlserver://",
];

fn push_drizzle_finding(out: &mut Vec<ToolConfigFinding>, node: Node<'_>, message: String) {
    out.push(ToolConfigFinding {
        violation: ToolConfigViolation::Sql(SqlToolConfigIssue::Drizzle(
            DrizzleConfigIssue::HardcodedDatabaseCredentials,
        )),
        span: node_span(node),
        message,
    });
}

fn after_db_scheme_has_creds(low: &str, after_scheme: &str) -> bool {
    let authority = after_scheme.split('/').next().unwrap_or("");
    let has_basic = authority
        .split_once('@')
        .is_some_and(|(userinfo, _)| userinfo.contains(':'));
    has_basic || low.contains("authtoken=") || low.contains("password=")
}

fn url_contains_inline_credentials(unquoted: &str) -> bool {
    let low = unquoted.trim().to_ascii_lowercase();
    if low.contains("${process.env") || low.contains("${import.meta.env") {
        return false;
    }
    DB_URL_SCHEMES.iter().any(|scheme| {
        low.strip_prefix(scheme)
            .is_some_and(|rest| after_db_scheme_has_creds(&low, rest))
    })
}

fn check_drizzle_pair(node: Node<'_>, source: &str, out: &mut Vec<ToolConfigFinding>) {
    let (Some(k), Some(v)) = (
        node.child_by_field_name("key"),
        node.child_by_field_name("value"),
    ) else {
        return;
    };
    let key_name = node_text(k, source).trim_matches(['"', '\'']);
    if matches!(key_name, "password" | "authToken")
        && matches!(v.kind(), "string" | "template_string")
        && !node_text(v, source).contains("process.env")
    {
        push_drizzle_finding(
            out,
            node,
            format!(
                "hardcoded `{key_name}` literal in `drizzle.config.*`; read from `process.env`"
            ),
        );
    }
}

fn walk_drizzle_config(node: Node<'_>, source: &str, out: &mut Vec<ToolConfigFinding>) {
    match node.kind() {
        "string" | "template_string" => {
            let inner = node_text(node, source).trim_matches(['"', '\'', '`']);
            if url_contains_inline_credentials(inner) {
                push_drizzle_finding(
                    out,
                    node,
                    String::from(
                        "hardcoded database URL with inline credentials in `drizzle.config.*`",
                    ),
                );
            }
        }
        "pair" => check_drizzle_pair(node, source, out),
        _ => {}
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_drizzle_config(ch, source, out);
    }
}

/// Parses and audits `drizzle.config.{ts,js,mjs}` for inline database credentials (`GF081`).
pub(super) fn audit_drizzle_config(source: &str, doc: &mut UastDocument) {
    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    if let Ok(tree) = parse_with_language(&ts_lang, source) {
        let root = tree.root_node();
        doc.syntax_errors = collect_syntax_errors(root);
        walk_drizzle_config(root, source, &mut doc.tool_config_findings);
    }
}
