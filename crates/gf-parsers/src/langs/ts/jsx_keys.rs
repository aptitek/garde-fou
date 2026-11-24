//! React iterator `key` verification (`REACT-JSX-KEY`): flags missing `key`, shorthand
//! `<>...</>` fragments in `.map`/`flatMap`/`Array.from`, and unstable array-index `key={idx}`.

use gf_core::{LangRuleFinding, ReactViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const FN_BOUNDARY_KINDS: &[&str] = &[
    "function_declaration",
    "arrow_function",
    "function_expression",
    "method_definition",
];

fn peel_parens(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            break;
        };
        node = inner;
    }
    node
}

fn iterator_callback_node<'a>(call_node: Node<'a>, source: &str) -> Option<Node<'a>> {
    let callee = call_node.child_by_field_name("function")?;
    if callee.kind() != "member_expression" {
        return None;
    }
    let prop = callee.child_by_field_name("property")?;
    let method = node_text(prop, source).trim();
    let cb_arg_idx = match method {
        "map" | "flatMap" => 0,
        "from" => {
            let obj = callee.child_by_field_name("object")?;
            if node_text(obj, source).trim() != "Array" {
                return None;
            }
            1
        }
        _ => return None,
    };
    let args = call_node.child_by_field_name("arguments")?;
    let raw_cb = args.named_child(cb_arg_idx)?;
    let cb = peel_parens(raw_cb);
    matches!(cb.kind(), "arrow_function" | "function_expression").then_some(cb)
}

fn extract_second_param_ident<'a>(cb_node: Node<'_>, source: &'a str) -> Option<&'a str> {
    let second = cb_node.child_by_field_name("parameters")?.named_child(1)?;
    let ident = second
        .child_by_field_name("pattern")
        .filter(|p| p.kind() == "identifier")?;
    Some(node_text(ident, source).trim())
}

fn expr_references_ident(node: Node<'_>, ident: &str, source: &str) -> bool {
    if node.kind() == "identifier" && node_text(node, source).trim() == ident {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|ch| expr_references_ident(ch, ident, source))
}

fn find_key_attribute<'a>(tag_holder: Node<'a>, source: &str) -> Option<Node<'a>> {
    let mut cursor = tag_holder.walk();
    tag_holder.named_children(&mut cursor).find(|&child| {
        child.kind() == "jsx_attribute"
            && child
                .named_child(0)
                .is_some_and(|name| node_text(name, source).trim() == "key")
    })
}

fn is_shorthand_jsx_fragment(node: Node<'_>) -> bool {
    node.kind() == "jsx_element"
        && node
            .child_by_field_name("open_tag")
            .is_some_and(|open| open.child_by_field_name("name").is_none())
}

fn verify_element_key_prop(
    expr: Node<'_>,
    index_ident: Option<&str>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let tag_holder = if expr.kind() == "jsx_element" {
        let Some(open) = expr.child_by_field_name("open_tag") else {
            return;
        };
        open
    } else {
        expr
    };
    let Some(key_attr) = find_key_attribute(tag_holder, source) else {
        out.push(LangRuleFinding::new(
            ReactViolation::MissingOrIndexJsxKey,
            node_span(tag_holder),
            "JSX element returned from `.map`/`flatMap`/`Array.from` is missing a `key` prop",
        ));
        return;
    };
    if let (Some(idx_name), Some(val_node)) = (index_ident, key_attr.named_child(1))
        && expr_references_ident(val_node, idx_name, source)
    {
        out.push(LangRuleFinding::new(
            ReactViolation::MissingOrIndexJsxKey,
            node_span(key_attr),
            format!(
                "array index `{idx_name}` used in JSX `key` causes broken reconciliation on reorder/delete"
            ),
        ));
    }
}

fn inspect_returned_jsx_expr(
    raw_expr: Node<'_>,
    index_ident: Option<&str>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let expr = peel_parens(raw_expr);
    match expr.kind() {
        "ternary_expression" => {
            for field in ["consequence", "alternative"] {
                if let Some(arm) = expr.child_by_field_name(field) {
                    inspect_returned_jsx_expr(arm, index_ident, source, out);
                }
            }
        }
        "binary_expression" => {
            if let Some(rhs) = expr.child_by_field_name("right") {
                inspect_returned_jsx_expr(rhs, index_ident, source, out);
            }
        }
        _ if is_shorthand_jsx_fragment(expr) => out.push(LangRuleFinding::new(
            ReactViolation::MissingOrIndexJsxKey,
            node_span(expr),
            "shorthand Fragment `<>...</>` returned from iterator cannot receive a `key` prop; use `<Fragment key={item.id}>`",
        )),
        "jsx_element" | "jsx_self_closing_element" => {
            verify_element_key_prop(expr, index_ident, source, out);
        }
        _ => {}
    }
}

fn collect_callback_returns(
    node: Node<'_>,
    index_ident: Option<&str>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if node.kind() == "return_statement" {
        if let Some(val) = node.named_child(0) {
            inspect_returned_jsx_expr(val, index_ident, source, out);
        }
        return;
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        if !FN_BOUNDARY_KINDS.contains(&ch.kind()) {
            collect_callback_returns(ch, index_ident, source, out);
        }
    }
}

/// Inspects JSX iterator callbacks for missing `key`, shorthand `<>`, and index `key={idx}` (`REACT-JSX-KEY`).
pub(super) fn extract_jsx_iterator_keys(
    root: Node<'_>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if root.kind() == "call_expression"
        && let Some(cb) = iterator_callback_node(root, source)
        && let Some(body) = cb.child_by_field_name("body")
    {
        let idx_ident = extract_second_param_ident(cb, source);
        if body.kind() == "statement_block" {
            collect_callback_returns(body, idx_ident, source, out);
        } else {
            inspect_returned_jsx_expr(body, idx_ident, source, out);
        }
    }
    let mut cursor = root.walk();
    for ch in root.children(&mut cursor) {
        extract_jsx_iterator_keys(ch, source, out);
    }
}
