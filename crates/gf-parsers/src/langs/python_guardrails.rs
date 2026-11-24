//! Tier 1 native Python AST guardrails (`GF080`) transplanted from Ruff / Bugbear / Bandit.
//!
//! Operates directly on the `Tree-sitter` Python CST with zero external process calls:
//! `PY-B006`, `PY-E722`, `PY-B001`, `PY-S102`, `PY-S301`, `PY-S602`, `PY-ANN401`, `PY-T201`.

use gf_core::{DomainKind, LangRuleFinding, UastDocument};
use tree_sitter::Node;

use super::python_calls::{compact_callee, inspect_py_call, push_py_finding};
use crate::cst::{field_name_string, node_span, node_text};

const MUTABLE_CALL_CONSTRUCTORS: &[&str] = &[
    "list",
    "dict",
    "set",
    "deque",
    "collections.deque",
    "defaultdict",
    "collections.defaultdict",
    "Counter",
    "collections.Counter",
    "OrderedDict",
    "collections.OrderedDict",
];

fn is_mutable_default_expr(val: Node<'_>, source: &str) -> bool {
    if matches!(
        val.kind(),
        "list"
            | "dictionary"
            | "set"
            | "list_comprehension"
            | "dictionary_comprehension"
            | "set_comprehension"
    ) {
        return true;
    }
    val.kind() == "call"
        && MUTABLE_CALL_CONSTRUCTORS.contains(&compact_callee(val, source).as_str())
}

fn inspect_default_param(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(val) = node.child_by_field_name("value") else {
        return;
    };
    if !is_mutable_default_expr(val, source) {
        return;
    }
    let param = field_name_string(node, "name", "<param>", source);
    push_py_finding(
        out,
        "PY-B006",
        node_span(node),
        (
            format!("mutable default argument in parameter `{param}` is shared across calls"),
            "Default the parameter to `None` (`param: list[T] | None = None`) and allocate a fresh container inside the function body.",
        ),
    );
}

fn is_noop_stmt(stmt: Node<'_>) -> bool {
    if stmt.kind() == "pass_statement" || stmt.kind() == "comment" {
        return true;
    }
    stmt.kind() == "expression_statement"
        && stmt
            .named_child(0)
            .is_some_and(|c| matches!(c.kind(), "ellipsis" | "string"))
}

fn is_silent_except_block(block: Node<'_>) -> bool {
    let mut cursor = block.walk();
    let stmts: Vec<Node<'_>> = block
        .named_children(&mut cursor)
        .filter(|n| n.kind() != "comment")
        .collect();
    !stmts.is_empty() && stmts.into_iter().all(is_noop_stmt)
}

fn inspect_except_clause(node: Node<'_>, out: &mut Vec<LangRuleFinding>) {
    let has_exc_type = node.child_by_field_name("value").is_some();
    let mut cursor = node.walk();
    let body_block = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "block");
    if !has_exc_type {
        push_py_finding(
            out,
            "PY-E722",
            node_span(node),
            (
                String::from("bare `except:` catches `SystemExit` and `KeyboardInterrupt`"),
                "Catch a specific exception class (e.g. `except ValueError as err:`) and handle or re-raise it explicitly.",
            ),
        );
    } else if body_block.is_some_and(is_silent_except_block) {
        push_py_finding(
            out,
            "PY-B001",
            node_span(node),
            (
                String::from("silent `except` handler swallows exceptions with `pass`/`...`"),
                "Log context, return an explicit error result, or re-raise with `raise ... from err` instead of swallowing exceptions.",
            ),
        );
    }
}

fn has_type_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "type" {
            return true;
        }
        node = parent;
    }
    false
}

/// Extracts Tier 1 Python AST idiom & security findings (`GF080`) into `doc.lang_rule_findings`.
pub fn extract_python_guardrails(
    root: Node<'_>,
    source: &str,
    domain: DomainKind,
    doc: &mut UastDocument,
) {
    walk_guardrails(root, source, domain, &mut doc.lang_rule_findings);
}

fn walk_guardrails(
    node: Node<'_>,
    source: &str,
    domain: DomainKind,
    out: &mut Vec<LangRuleFinding>,
) {
    match node.kind() {
        "default_parameter" | "typed_default_parameter" => {
            inspect_default_param(node, source, out);
        }
        "except_clause" | "except_group_clause" => inspect_except_clause(node, out),
        "call" => inspect_py_call(node, source, domain, out),
        "identifier"
            if domain == DomainKind::Code
                && node_text(node, source) == "Any"
                && has_type_ancestor(node) =>
        {
            push_py_finding(
                out,
                "PY-ANN401",
                node_span(node),
                (
                    String::from("explicit `Any` annotation weakens static typing in `Code`"),
                    "Replace `Any` with concrete types, `TypeVar`, `TypedDict`, `dataclass`, or a `Protocol`.",
                ),
            );
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_guardrails(child, source, domain, out);
    }
}
