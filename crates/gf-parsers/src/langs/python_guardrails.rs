//! Tier 1 native Python AST guardrails (`GF080` & `GF011` `SilentErrorSwallow`).

use gf_core::{
    DomainKind, EffectSighting, EffectSightingKind, ForbiddenRuleKind, LangRuleFinding,
    UastDocument, UastKind, UastNode,
};
use tree_sitter::Node;

use super::python_calls::{PyFindingSpec, compact_callee, inspect_py_call, push_py_finding};
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

const FALLBACK_RETURN_LITERALS: &[&str] = &["", "None", "[]", "{}", "\"\"", "''", "False", "0"];

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
        PyFindingSpec {
            code: "PY-B006",
            span: node_span(node),
            message: format!(
                "mutable default argument in parameter `{param}` is shared across calls"
            ),
            remediation: "Default the parameter to `None` (`param: list[T] | None = None`) and allocate a fresh container inside the function body.",
        },
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

fn is_silent_fallback_return_block(block: Node<'_>, source: &str) -> bool {
    let mut cursor = block.walk();
    let stmts: Vec<Node<'_>> = block
        .named_children(&mut cursor)
        .filter(|n| n.kind() != "comment")
        .collect();
    if stmts.is_empty() {
        return false;
    }
    let mut saw_fallback_return = false;
    for stmt in stmts {
        if is_noop_stmt(stmt) {
            continue;
        }
        if stmt.kind() != "return_statement" {
            return false;
        }
        let rhs = node_text(stmt, source)
            .trim()
            .strip_prefix("return")
            .unwrap_or("")
            .trim();
        if !FALLBACK_RETURN_LITERALS.contains(&rhs) {
            return false;
        }
        saw_fallback_return = true;
    }
    saw_fallback_return
}

fn inspect_except_clause(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let has_exc_type = node.child_by_field_name("value").is_some();
    let mut cursor = node.walk();
    let body_block = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "block");
    let span = node_span(node);
    if !has_exc_type {
        push_py_finding(
            &mut doc.lang_rule_findings,
            PyFindingSpec {
                code: "PY-E722",
                span,
                message: String::from(
                    "bare `except:` catches `SystemExit` and `KeyboardInterrupt`",
                ),
                remediation: "Catch a specific exception class (e.g. `except ValueError as err:`) and handle or re-raise it explicitly.",
            },
        );
    } else if body_block.is_some_and(is_silent_except_block) {
        push_py_finding(
            &mut doc.lang_rule_findings,
            PyFindingSpec {
                code: "PY-B001",
                span,
                message: String::from(
                    "silent `except` handler swallows exceptions with `pass`/`...`",
                ),
                remediation: "Log context, return an explicit error result, or re-raise with `raise ... from err` instead of swallowing exceptions.",
            },
        );
    } else if body_block.is_some_and(|b| is_silent_fallback_return_block(b, source)) {
        doc.root.children.push(UastNode::leaf(
            UastKind::ForbiddenCall {
                rule: ForbiddenRuleKind::SilentErrorSwallow,
                symbol: String::from("except: return fallback"),
            },
            span,
        ));
        doc.effect_sightings.push(EffectSighting {
            kind: EffectSightingKind::SilentErrorSwallow,
            detail: String::from("silent `except` returning sentinel fallback"),
            span,
        });
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

/// Extracts Tier 1 Python AST idiom, security, and silent error-swallowing findings.
pub fn extract_python_guardrails(
    root: Node<'_>,
    source: &str,
    domain: DomainKind,
    doc: &mut UastDocument,
) {
    walk_guardrails(root, source, domain, doc);
}

fn walk_guardrails(node: Node<'_>, source: &str, domain: DomainKind, doc: &mut UastDocument) {
    match node.kind() {
        "default_parameter" | "typed_default_parameter" => {
            inspect_default_param(node, source, &mut doc.lang_rule_findings);
        }
        "except_clause" | "except_group_clause" => inspect_except_clause(node, source, doc),
        "call" => inspect_py_call(node, source, domain, doc),
        "identifier"
            if domain == DomainKind::Code
                && node_text(node, source) == "Any"
                && has_type_ancestor(node) =>
        {
            push_py_finding(
                &mut doc.lang_rule_findings,
                PyFindingSpec {
                    code: "PY-ANN401",
                    span: node_span(node),
                    message: String::from(
                        "explicit `Any` annotation weakens static typing in `Code`",
                    ),
                    remediation: "Replace `Any` with concrete types, `TypeVar`, `TypedDict`, `dataclass`, or a `Protocol`.",
                },
            );
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_guardrails(child, source, domain, doc);
    }
}
