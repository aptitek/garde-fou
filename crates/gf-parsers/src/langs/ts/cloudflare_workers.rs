//! Cloudflare Workers V8 isolate purity, Node API restrictions, D1 limits/N+1, and R2 safety (`CF-*`).

use super::style_and_i18n::unquote_literal;
use crate::cst::{node_span, node_text};
use gf_core::{CloudflareViolation, LangRuleFinding, Span, UastDocument, WebEdgeSummary};
use std::collections::BTreeSet;
use tree_sitter::Node;

#[rustfmt::skip]
const FORBIDDEN_NODE_CORES: &[&str] = &[
    "child_process", "cluster", "dgram", "fs", "fs/promises", "net", "tls", "v8",
];
#[rustfmt::skip]
const IMPURE_GLOBAL_CALLS: &[&str] = &[
    "Date.now", "crypto.getRandomValues", "crypto.randomUUID", "fetch", "setInterval", "setTimeout",
];
const D1_EXEC_TERMINALS: &[&str] = &["run", "all", "first", "raw"];

fn check_worker_imports(doc: &mut UastDocument) {
    let we = doc.web_edge.get_or_insert_with(WebEdgeSummary::default);
    for imp in &doc.imports {
        let m = imp.module_source.trim();
        let bare = m.strip_prefix("node:").unwrap_or(m);
        if FORBIDDEN_NODE_CORES.contains(&bare) {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                CloudflareViolation::ForbiddenNodeApi,
                imp.span,
                format!("Node module `{m}` is incompatible with the Cloudflare Workers runtime"),
            ));
        } else if m.starts_with("node:") {
            we.node_colon_imports.push((String::from(m), imp.span));
        }
    }
}

#[rustfmt::skip]
fn collect_env_and_globals(node: Node<'_>, source: &str, env_keys: &mut BTreeSet<String>, mutable_globals: &mut BTreeSet<String>) {
    match node.kind() {
        "interface_declaration" | "type_alias_declaration"
            if node.child_by_field_name("name").is_some_and(|n| node_text(n, source).trim() == "Env") =>
        {
            return collect_field_names(node, "property_signature", source, env_keys);
        }
        "lexical_declaration" | "variable_declaration"
            if node.parent().is_some_and(|p| matches!(p.kind(), "program" | "export_statement")) =>
        {
            let raw = node_text(node, source).trim_start();
            if raw.starts_with("let ") || raw.starts_with("var ") {
                collect_field_names(node, "variable_declarator", source, mutable_globals);
            }
            return;
        }
        _ => {}
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        collect_env_and_globals(ch, source, env_keys, mutable_globals);
    }
}

fn collect_field_names(node: Node<'_>, kind: &str, source: &str, out: &mut BTreeSet<String>) {
    if node.kind() == kind
        && let Some(n) = node.child_by_field_name("name")
    {
        out.insert(String::from(node_text(n, source).trim()));
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        collect_field_names(ch, kind, source, out);
    }
}

#[derive(Clone, Copy)]
struct WorkerWalkState<'a> {
    source: &'a str,
    in_callable: bool,
    in_loop: bool,
    mutable_globals: &'a BTreeSet<String>,
}

#[rustfmt::skip]
fn check_d1_or_r2_arg(callee: &str, prop: &str, arg0_raw: &str, span: Span) -> Option<LangRuleFinding> {
    if prop == "prepare" {
        let q_count = arg0_raw.bytes().filter(|&b| b == b'?').count();
        return (q_count > 100).then(|| {
            LangRuleFinding::new(
                CloudflareViolation::D1NPlusOneOrLimit,
                span,
                format!("D1 prepared statement binds {q_count} `?` parameters (max 100)"),
            )
        });
    }
    if prop == "exec" && callee.starts_with("env.") && (arg0_raw.contains("${") || arg0_raw.contains('+')) {
        return Some(LangRuleFinding::new(
            CloudflareViolation::D1NPlusOneOrLimit,
            span,
            String::from("dynamic string interpolation in `env.DB.exec(...)` is unsafe"),
        ));
    }
    (matches!(prop, "put" | "get")
        && callee.starts_with("env.")
        && unquote_literal(arg0_raw).is_some_and(|k| k.contains("..")))
    .then(|| {
        LangRuleFinding::new(
            CloudflareViolation::R2StreamAndKeySafety,
            span,
            format!("unsanitized `..` path traversal in R2 `.{prop}()` object key"),
        )
    })
}

#[rustfmt::skip]
fn check_call_node(node: Node<'_>, st: WorkerWalkState<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else { return };
    let callee = node_text(func, st.source).trim();
    let span = node_span(node);
    if !st.in_callable && (IMPURE_GLOBAL_CALLS.contains(&callee) || callee.starts_with("crypto.subtle.")) {
        out.push(LangRuleFinding::new(
            CloudflareViolation::GlobalScopeImpure,
            span,
            format!("`{callee}(...)` is forbidden in Cloudflare Workers global isolate scope"),
        ));
    }
    let Some(prop_node) = (func.kind() == "member_expression")
        .then(|| func.child_by_field_name("property"))
        .flatten()
    else {
        return;
    };
    let prop = node_text(prop_node, st.source).trim();
    if st.in_loop && D1_EXEC_TERMINALS.contains(&prop) && callee.contains(".prepare(") {
        out.push(LangRuleFinding::new(
            CloudflareViolation::D1NPlusOneOrLimit,
            span,
            format!("D1 `.prepare(...).{prop}()` in loop causes N+1 queries; use `env.DB.batch([...])`"),
        ));
    }
    if let Some(arg0) = node.child_by_field_name("arguments").and_then(|a| a.named_child(0))
        && let Some(finding) = check_d1_or_r2_arg(callee, prop, node_text(arg0, st.source), span)
    {
        out.push(finding);
    }
}

#[rustfmt::skip]
fn check_assignment_or_update(node: Node<'_>, st: WorkerWalkState<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(l) = node.child_by_field_name("left").or_else(|| node.child_by_field_name("argument")) else {
        return;
    };
    let l_txt = node_text(l, st.source).trim();
    if matches!(l_txt, "request.cf" | "req.cf") || l_txt.starts_with("request.cf.") || l_txt.starts_with("req.cf.") {
        out.push(LangRuleFinding::new(
            CloudflareViolation::R2StreamAndKeySafety,
            node_span(node),
            format!("`{l_txt}` is read-only at runtime in Cloudflare Workers"),
        ));
    } else if st.in_callable && st.mutable_globals.contains(l_txt) {
        out.push(LangRuleFinding::new(
            CloudflareViolation::GlobalScopeImpure,
            node_span(node),
            format!("mutating top-level variable `{l_txt}` leaks state across requests in a V8 isolate"),
        ));
    }
}

#[rustfmt::skip]
fn visit_worker_node(node: Node<'_>, st: WorkerWalkState<'_>, doc: &mut UastDocument) {
    match node.kind() {
        "call_expression" => check_call_node(node, st, &mut doc.lang_rule_findings),
        "new_expression"
            if !st.in_callable
                && node.child_by_field_name("constructor").is_some_and(|c| node_text(c, st.source).trim() == "Date") =>
        {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                CloudflareViolation::GlobalScopeImpure,
                node_span(node),
                String::from("`new Date()` in Workers global scope freezes clock to isolate startup"),
            ));
        }
        "member_expression" => {
            if let (Some(obj), Some(prop)) = (node.child_by_field_name("object"), node.child_by_field_name("property"))
                && node_text(obj, st.source).trim() == "env"
            {
                let b_name = node_text(prop, st.source).trim();
                doc.web_edge
                    .get_or_insert_with(WebEdgeSummary::default)
                    .env_bindings_accessed
                    .push((String::from(b_name), node_span(node)));
            }
        }
        "assignment_expression" | "augmented_assignment_expression" | "update_expression" => {
            check_assignment_or_update(node, st, &mut doc.lang_rule_findings);
        }
        _ => {}
    }
}

fn walk_worker_ast(node: Node<'_>, st: WorkerWalkState<'_>, doc: &mut UastDocument) {
    let is_fn = matches!(
        node.kind(),
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    );
    let is_loop = matches!(
        node.kind(),
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
    );
    let next_st = WorkerWalkState {
        in_callable: st.in_callable || is_fn,
        in_loop: st.in_loop || is_loop,
        ..st
    };
    visit_worker_node(node, next_st, doc);
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk_worker_ast(ch, next_st, doc);
    }
}

/// Evaluates Cloudflare Workers, D1, and R2 rules (`CF-*`) on a TypeScript/JavaScript CST.
pub fn extract_cloudflare_workers_rules(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    check_worker_imports(doc);
    let (mut env_keys, mut mutable_globals) = (BTreeSet::new(), BTreeSet::new());
    collect_env_and_globals(root, source, &mut env_keys, &mut mutable_globals);
    let st = WorkerWalkState {
        source,
        in_callable: false,
        in_loop: false,
        mutable_globals: &mutable_globals,
    };
    walk_worker_ast(root, st, doc);
    if !env_keys.is_empty() {
        let we = doc.web_edge.get_or_insert_with(WebEdgeSummary::default);
        for (acc, span) in &we.env_bindings_accessed {
            if !env_keys.contains(acc.as_str()) {
                doc.lang_rule_findings.push(LangRuleFinding::new(
                    CloudflareViolation::BindingCoherence,
                    *span,
                    format!("`env.{acc}` is not declared in `Env`"),
                ));
            }
        }
        we.documented_env_keys = env_keys;
    }
}
