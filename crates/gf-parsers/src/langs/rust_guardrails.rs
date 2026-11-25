//! Rust AST guardrails for `GF010` (`ForbiddenRuleKind`) and `GF011` (`SilentErrorSwallow`).

use gf_core::{EffectSightingKind, ForbiddenRuleKind, Span, UastDocument, UastKind, UastNode};
use tree_sitter::Node;

use super::rust_effects::{inspect_rust_effect_node, push_effect};
use super::sql::embedded_sql::inspect_rust_embedded_sql_node;
use crate::cst::{field_name_string, node_span, node_text};

fn push_forbidden(doc: &mut UastDocument, rule: ForbiddenRuleKind, symbol: &str, span: Span) {
    doc.root.children.push(UastNode::leaf(
        UastKind::ForbiddenCall {
            rule,
            symbol: String::from(symbol),
        },
        span,
    ));
}

fn has_unsafe_token_child(node: Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|ch| {
        (!ch.is_named() && node_text(ch, source) == "unsafe")
            || (ch.kind() == "function_modifiers"
                && node_text(ch, source)
                    .split_whitespace()
                    .any(|w| w == "unsafe"))
    })
}

fn inspect_unsafe_construct(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let span = node_span(node);
    match node.kind() {
        "unsafe_block" => push_forbidden(doc, ForbiddenRuleKind::Unsafe, "unsafe block", span),
        "foreign_mod_item" => push_forbidden(doc, ForbiddenRuleKind::Unsafe, "extern block", span),
        "function_item" | "function_signature_item" if has_unsafe_token_child(node, source) => {
            push_forbidden(doc, ForbiddenRuleKind::Unsafe, "unsafe fn", span);
        }
        "trait_item" if has_unsafe_token_child(node, source) => {
            push_forbidden(doc, ForbiddenRuleKind::Unsafe, "unsafe trait", span);
        }
        "impl_item" if has_unsafe_token_child(node, source) => {
            push_forbidden(doc, ForbiddenRuleKind::Unsafe, "unsafe impl", span);
        }
        _ => {}
    }
}

fn inspect_macro_call(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    let Some(mac_node) = node.child_by_field_name("macro") else {
        return;
    };
    let raw = node_text(mac_node, source).trim();
    let short = raw.rsplit("::").next().unwrap_or(raw).trim();
    let span = node_span(node);
    match short {
        "todo" => push_forbidden(doc, ForbiddenRuleKind::Todo, "todo!", span),
        "unimplemented" => {
            push_forbidden(
                doc,
                ForbiddenRuleKind::Unimplemented,
                "unimplemented!",
                span,
            );
        }
        "dbg" => push_forbidden(doc, ForbiddenRuleKind::Dbg, "dbg!", span),
        "panic" | "unreachable" if !in_cfg_test => {
            push_forbidden(doc, ForbiddenRuleKind::Panic, &format!("{short}!"), span);
        }
        "println" | "print" | "eprintln" | "eprint" => {
            push_effect(
                doc,
                EffectSightingKind::ConsoleOutput,
                &format!("{short}!"),
                span,
            );
        }
        _ => {}
    }
}

fn is_wildcard_discard_closure(call_node: Node<'_>, source: &str) -> bool {
    let Some(args) = call_node.child_by_field_name("arguments") else {
        return false;
    };
    let mut cursor = args.walk();
    let Some(first) = args.named_children(&mut cursor).next() else {
        return false;
    };
    first.kind() == "closure_expression"
        && first.child_by_field_name("parameters").is_some_and(|p| {
            let clean = node_text(p, source).trim().trim_matches('|').trim();
            clean == "_"
        })
}

fn inspect_method_call(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    if in_cfg_test {
        return;
    }
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    if func.kind() != "field_expression" {
        return;
    }
    let method = field_name_string(func, "field", "", source);
    let span = node_span(node);
    match method.as_str() {
        "unwrap" | "unwrap_err" => {
            push_forbidden(
                doc,
                ForbiddenRuleKind::Unwrap,
                &format!(".{method}()"),
                span,
            );
        }
        "expect" | "expect_err" => {
            push_forbidden(
                doc,
                ForbiddenRuleKind::Expect,
                &format!(".{method}(...)"),
                span,
            );
        }
        "unwrap_or_else" if is_wildcard_discard_closure(node, source) => {
            let sym = ".unwrap_or_else(|_| ...)";
            push_forbidden(doc, ForbiddenRuleKind::SilentErrorSwallow, sym, span);
            push_effect(doc, EffectSightingKind::SilentErrorSwallow, sym, span);
        }
        _ => {}
    }
}

fn inspect_let_discard(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    if in_cfg_test {
        return;
    }
    let is_underscore = node
        .child_by_field_name("pattern")
        .is_some_and(|p| node_text(p, source).trim() == "_");
    let is_call_or_try = node
        .child_by_field_name("value")
        .is_some_and(|v| matches!(v.kind(), "call_expression" | "try_expression"));
    if is_underscore && is_call_or_try {
        let span = node_span(node);
        push_forbidden(
            doc,
            ForbiddenRuleKind::SilentErrorSwallow,
            "let _ = ...",
            span,
        );
        push_effect(
            doc,
            EffectSightingKind::SilentErrorSwallow,
            "let _ = ...",
            span,
        );
    }
}

/// Inspects a single Rust CST node for `GF010`, `GF011`, `GF078`, and inline module imports.
pub fn inspect_rust_guardrail_node(
    node: Node<'_>,
    source: &str,
    in_cfg_test: bool,
    doc: &mut UastDocument,
) {
    inspect_unsafe_construct(node, source, doc);
    inspect_rust_effect_node(node, source, in_cfg_test, doc);
    if !in_cfg_test {
        inspect_rust_embedded_sql_node(node, source, &mut doc.lang_rule_findings);
    }
    match node.kind() {
        "macro_invocation" => inspect_macro_call(node, source, in_cfg_test, doc),
        "call_expression" => inspect_method_call(node, source, in_cfg_test, doc),
        "let_declaration" => inspect_let_discard(node, source, in_cfg_test, doc),
        _ => {}
    }
}
