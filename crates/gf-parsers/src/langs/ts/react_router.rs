//! React Router v7 Data & Framework mode CST analyzer (`RR-*` — `GF080` / `GF072`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, ReactRouterViolation, RepoPath, Span, UastDocument};
use tree_sitter::Node;

use super::style_and_i18n::unquote_literal;
use crate::cst::{node_span, node_text};

const LOADER_NAMES: &[&str] = &["loader", "clientLoader"];
const CLIENT_FN_NAMES: &[&str] = &["clientLoader", "clientAction"];
const MUTATION_METHODS: &[&str] = &["insert", "update", "delete", "put", "destroy"];
const MUTATION_HTTP_VERBS: &[&str] = &["post", "put", "patch", "delete"];

fn is_server_only_specifier(spec: &str) -> bool {
    let clean = spec.trim();
    clean.ends_with(".server")
        || clean.contains(".server.")
        || clean.contains("/server/")
        || clean == "drizzle-orm"
        || clean.starts_with("drizzle-orm/")
}

fn is_react_router_route_path(path: &str) -> bool {
    matches!(path, "root.tsx" | "root.jsx")
        || path.ends_with("/root.tsx")
        || path.ends_with("/root.jsx")
        || path.starts_with("routes/")
        || path.contains("/routes/")
}

fn collect_server_imports(doc: &mut UastDocument, path_str: &str) -> BTreeSet<String> {
    let mut server_syms = BTreeSet::new();
    let is_client_or_ui = path_str.contains(".client.")
        || path_str.contains("/components/")
        || path_str.starts_with("components/");
    for imp in &doc.imports {
        if !is_server_only_specifier(&imp.module_source) {
            continue;
        }
        if is_client_or_ui {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                ReactRouterViolation::ServerLeakToClient,
                imp.span,
                format!(
                    "server-only module `{}` imported in client/UI module `{path_str}`",
                    imp.module_source
                ),
            ));
        }
        for sym in &imp.symbols {
            server_syms.insert(sym.clone());
        }
    }
    server_syms
}

#[rustfmt::skip]
fn check_loader_body_mutations(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "call_expression" => {
            if let Some(prop) = node
                .child_by_field_name("function")
                .filter(|f| f.kind() == "member_expression")
                .and_then(|f| f.child_by_field_name("property"))
            {
                let m = node_text(prop, source).trim();
                if MUTATION_METHODS.contains(&m) {
                    out.push(LangRuleFinding::new(
                        ReactRouterViolation::MutationInLoader,
                        node_span(node),
                        format!("data mutation `.{m}(...)` is forbidden inside `loader`/`clientLoader`; move to `action`"),
                    ));
                }
            }
        }
        "pair" => {
            if let (Some(k), Some(v)) = (node.child_by_field_name("key"), node.child_by_field_name("value"))
                && node_text(k, source).trim_matches(['"', '\'']).eq_ignore_ascii_case("method")
                && let Some(verb) = unquote_literal(node_text(v, source))
                && MUTATION_HTTP_VERBS.contains(&verb.trim().to_ascii_lowercase().as_str())
            {
                out.push(LangRuleFinding::new(
                    ReactRouterViolation::MutationInLoader,
                    node_span(node),
                    format!("HTTP `{verb}` mutation inside `loader`/`clientLoader`; move to `action`"),
                ));
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        check_loader_body_mutations(ch, source, out);
    }
}

#[rustfmt::skip]
fn check_client_scope_server_leaks(node: Node<'_>, source: &str, server_syms: &BTreeSet<String>, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "call_expression" => {
            if node.child_by_field_name("function").is_some_and(|f| node_text(f, source).trim() == "import")
                && let Some(arg0) = node.child_by_field_name("arguments").and_then(|a| a.named_child(0))
                && let Some(spec) = unquote_literal(node_text(arg0, source))
                && is_server_only_specifier(spec)
            {
                out.push(LangRuleFinding::new(
                    ReactRouterViolation::ServerLeakToClient,
                    node_span(node),
                    format!("dynamic import of server module `{spec}` inside client scope"),
                ));
            }
        }
        "identifier" => {
            let ident = node_text(node, source).trim();
            if server_syms.contains(ident) {
                out.push(LangRuleFinding::new(
                    ReactRouterViolation::ServerLeakToClient,
                    node_span(node),
                    format!("server binding `{ident}` referenced inside client scope"),
                ));
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        check_client_scope_server_leaks(ch, source, server_syms, out);
    }
}

fn callable_name_and_body<'a>(node: Node<'a>, source: &'a str) -> Option<(&'a str, Node<'a>)> {
    let name = node_text(node.child_by_field_name("name")?, source).trim();
    match node.kind() {
        "function_declaration" => Some((name, node.child_by_field_name("body")?)),
        "variable_declarator" => {
            let val = node.child_by_field_name("value")?;
            matches!(val.kind(), "arrow_function" | "function_expression").then_some((name, val))
        }
        _ => None,
    }
}

fn check_internal_anchor(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let tag_holder = match node.kind() {
        "jsx_element" => node.child_by_field_name("open_tag"),
        "jsx_self_closing_element" => Some(node),
        _ => None,
    };
    let Some(tag_holder) = tag_holder else {
        return;
    };
    if tag_holder
        .child_by_field_name("name")
        .is_none_or(|n| node_text(n, source).trim() != "a")
    {
        return;
    }
    let raw_tag = node_text(tag_holder, source);
    if ["download", "reloadDocument", "external", "_blank"]
        .iter()
        .any(|kw| raw_tag.contains(kw))
    {
        return;
    }
    let mut c = tag_holder.walk();
    for ch in tag_holder.named_children(&mut c) {
        if ch.kind() == "jsx_attribute"
            && ch
                .child(0)
                .is_some_and(|k| node_text(k, source).trim() == "href")
            && let Some(val) = ch.child(2)
            && let Some(href) = unquote_literal(node_text(val, source))
            && href.starts_with('/')
            && !href.starts_with("//")
        {
            out.push(LangRuleFinding::new(
                ReactRouterViolation::RawInternalAnchor,
                node_span(tag_holder),
                format!("raw internal `<a href=\"{href}\">` bypasses React Router client navigation; use `<Link to=\"{href}\">`"),
            ));
        }
    }
}

#[derive(Clone, Copy)]
struct RrWalkCtx<'a> {
    source: &'a str,
    server_syms: &'a BTreeSet<String>,
    check_anchors: bool,
}

fn walk_react_router_ast(node: Node<'_>, ctx: RrWalkCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    if let Some((fn_name, body)) = callable_name_and_body(node, ctx.source) {
        if LOADER_NAMES.contains(&fn_name) {
            check_loader_body_mutations(body, ctx.source, out);
        }
        let is_component = fn_name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase());
        if CLIENT_FN_NAMES.contains(&fn_name) || is_component {
            check_client_scope_server_leaks(body, ctx.source, ctx.server_syms, out);
        }
    }
    if ctx.check_anchors && matches!(node.kind(), "jsx_element" | "jsx_self_closing_element") {
        check_internal_anchor(node, ctx.source, out);
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_react_router_ast(ch, ctx, out);
    }
}

/// Evaluates React Router v7 rules (`RR-*`) on a TypeScript/TSX document.
#[rustfmt::skip]
pub fn extract_react_router_rules(root: Node<'_>, path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let path_str = path.as_str();
    let is_route = is_react_router_route_path(path_str);
    let imports_rr = doc
        .imports
        .iter()
        .any(|i| i.module_source == "react-router" || i.module_source.starts_with("@react-router/"));
    let server_syms = collect_server_imports(doc, path_str);

    if matches!(path.file_name(), "root.tsx" | "root.jsx")
        && !doc.exports.iter().any(|e| e.symbol == "ErrorBoundary")
    {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            ReactRouterViolation::RootMissingErrorBoundary,
            Span::FIRST,
            format!("React Router root module `{path_str}` does not export an `ErrorBoundary`"),
        ));
    }

    let ctx = RrWalkCtx {
        source,
        server_syms: &server_syms,
        check_anchors: is_route || imports_rr,
    };
    walk_react_router_ast(root, ctx, &mut doc.lang_rule_findings);
}
