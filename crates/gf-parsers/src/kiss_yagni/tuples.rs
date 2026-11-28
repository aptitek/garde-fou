//! Anonymous tuple arity and unsafe global tuple unpacking extractors (`GF073`)
//! across Rust, Python, TypeScript/JavaScript, and C# CSTs.

use gf_core::{KissYagniFinding, KissYagniKind, Language, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const PY_SCOPES: &[&str] = &["function_definition", "class_definition", "lambda"];
#[rustfmt::skip]
const TS_SCOPES: &[&str] = &[
    "function_declaration", "function_expression", "arrow_function",
    "method_definition", "class_declaration", "class", "static_block",
];
#[rustfmt::skip]
const CS_SCOPES: &[&str] = &[
    "method_declaration", "local_function_statement", "constructor_declaration",
    "destructor_declaration", "lambda_expression", "anonymous_method_expression",
    "accessor_declaration", "class_declaration", "struct_declaration", "record_declaration",
];

fn has_enclosing_scope(node: Node<'_>, scopes: &[&str]) -> bool {
    let mut cur = node.parent();
    while let Some(parent) = cur {
        if scopes.contains(&parent.kind()) {
            return true;
        }
        cur = parent.parent();
    }
    false
}

fn named_arity(node: Node<'_>) -> u32 {
    u32::try_from(node.named_child_count()).unwrap_or(u32::MAX)
}

fn push_tuple(doc: &mut UastDocument, arity: u32, detail: String, node: Node<'_>) {
    if arity >= 1 {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::AnonymousTuple,
            measured: arity,
            detail,
            span: node_span(node),
        });
    }
}

fn push_global_unpack(doc: &mut UastDocument, detail: String, node: Node<'_>) {
    doc.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::GlobalTupleUnpacking,
        measured: 1,
        detail,
        span: node_span(node),
    });
}

fn check_rust_tuples(node: Node<'_>, doc: &mut UastDocument) {
    let label = match node.kind() {
        "tuple_type" => "tuple type",
        "tuple_expression" => "tuple expression",
        "tuple_pattern" => "tuple pattern",
        _ => return,
    };
    let arity = named_arity(node);
    let detail = format!("anonymous Rust {label} with {arity} elements");
    push_tuple(doc, arity, detail, node);
}

fn py_generic_tuple_arity(node: Node<'_>, source: &str) -> Option<u32> {
    let base = node
        .child_by_field_name("value")
        .or_else(|| node.named_child(0))?;
    if !matches!(node_text(base, source).trim(), "tuple" | "Tuple") {
        return None;
    }
    if node.kind() == "generic_type" {
        let params = node.named_child(1)?;
        if params.kind() != "type_parameter" {
            return None;
        }
        let mut cur = params.walk();
        let count = params
            .named_children(&mut cur)
            .filter(|c| c.kind() != "ellipsis" && node_text(*c, source).trim() != "...")
            .count();
        return Some(u32::try_from(count).unwrap_or(u32::MAX));
    }
    let mut cur = node.walk();
    let count = node
        .children_by_field_name("subscript", &mut cur)
        .filter(|c| c.kind() != "ellipsis" && node_text(*c, source).trim() != "...")
        .count();
    (count > 0).then(|| u32::try_from(count).unwrap_or(u32::MAX))
}

fn check_py_assignment(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let is_tuple_lhs = node
        .child_by_field_name("left")
        .is_some_and(|l| matches!(l.kind(), "pattern_list" | "tuple_pattern"));
    if is_tuple_lhs && !has_enclosing_scope(node, PY_SCOPES) {
        let detail = format!(
            "module-level Python tuple unpacking `{}`",
            node_text(node, source).trim()
        );
        push_global_unpack(doc, detail, node);
    }
}

fn check_python_tuples(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "generic_type" | "subscript" => {
            if let Some(arity) = py_generic_tuple_arity(node, source) {
                let msg = format!("anonymous Python `tuple[...]` type with {arity} elements");
                push_tuple(doc, arity, msg, node);
            }
        }
        "tuple" if node.parent().is_none_or(|p| p.kind() != "except_clause") => {
            let arity = named_arity(node);
            let msg = format!("anonymous Python tuple literal with {arity} elements");
            push_tuple(doc, arity, msg, node);
        }
        "expression_list"
            if node
                .parent()
                .is_some_and(|p| p.kind() == "return_statement") =>
        {
            let arity = named_arity(node);
            if arity >= 2 {
                let msg = format!("anonymous Python tuple return with {arity} elements");
                push_tuple(doc, arity, msg, node);
            }
        }
        "pattern_list" | "tuple_pattern" => {
            let arity = named_arity(node);
            if arity >= 2 {
                let msg = format!("anonymous Python tuple unpacking with {arity} elements");
                push_tuple(doc, arity, msg, node);
            }
        }
        "assignment" => check_py_assignment(node, source, doc),
        _ => {}
    }
}

fn check_ts_global_unpack(node: Node<'_>, field: &str, source: &str, doc: &mut UastDocument) {
    let is_tuple_lhs = node
        .child_by_field_name(field)
        .is_some_and(|n| n.kind() == "array_pattern");
    if is_tuple_lhs && !has_enclosing_scope(node, TS_SCOPES) {
        let detail = format!(
            "module-level JS/TS tuple unpacking `{}`",
            node_text(node, source).trim()
        );
        push_global_unpack(doc, detail, node);
    }
}

fn check_ts_tuples(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "tuple_type" => {
            let arity = named_arity(node);
            let msg = format!("anonymous TypeScript tuple type with {arity} elements");
            push_tuple(doc, arity, msg, node);
        }
        "array_pattern" => {
            let arity = named_arity(node);
            let msg = format!("anonymous JS/TS tuple destructuring with {arity} elements");
            push_tuple(doc, arity, msg, node);
        }
        "variable_declarator" => check_ts_global_unpack(node, "name", source, doc),
        "assignment_expression" => check_ts_global_unpack(node, "left", source, doc),
        _ => {}
    }
}

fn has_cs_tuple_designation(node: Node<'_>) -> bool {
    if matches!(
        node.kind(),
        "parenthesized_variable_designation" | "tuple_pattern" | "tuple_expression"
    ) {
        return true;
    }
    let mut cur = node.walk();
    node.children(&mut cur).any(has_cs_tuple_designation)
}

fn check_csharp_tuples(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "tuple_type"
        | "tuple_expression"
        | "tuple_pattern"
        | "parenthesized_variable_designation" => {
            let arity = named_arity(node);
            push_tuple(
                doc,
                arity,
                format!("anonymous C# {} with {arity} elements", node.kind()),
                node,
            );
        }
        "assignment_expression" => {
            let is_tuple_lhs = node
                .child_by_field_name("left")
                .is_some_and(has_cs_tuple_designation);
            if is_tuple_lhs && !has_enclosing_scope(node, CS_SCOPES) {
                let detail = format!(
                    "top-level C# tuple unpacking `{}`",
                    node_text(node, source).trim()
                );
                push_global_unpack(doc, detail, node);
            }
        }
        "local_declaration_statement" if !has_enclosing_scope(node, CS_SCOPES) => {
            let text = node_text(node, source).trim();
            let has_tuple = node.named_child(0).is_some_and(has_cs_tuple_designation);
            if has_tuple
                && text
                    .split_once('=')
                    .is_some_and(|(lhs, _)| lhs.contains('('))
            {
                push_global_unpack(doc, format!("top-level C# tuple unpacking `{text}`"), node);
            }
        }
        _ => {}
    }
}

/// Inspects a single CST node for anonymous tuples and unsafe global tuple unpacking (`GF073`).
pub(super) fn check_tuple_kiss(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match doc.language {
        Language::Rust => check_rust_tuples(node, doc),
        Language::Python => check_python_tuples(node, source, doc),
        Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx => {
            check_ts_tuples(node, source, doc);
        }
        Language::CSharp => check_csharp_tuples(node, source, doc),
        _ => {}
    }
}
