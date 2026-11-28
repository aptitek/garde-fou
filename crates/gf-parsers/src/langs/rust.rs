//! Safe `Tree-sitter` extractor for Rust (`.rs`).

use gf_core::{CallableDecl, CallableKind, Language, RepoPath, TypeDecl, TypeKind, UastDocument};
use tree_sitter::Node;

use super::rust_effects::{extract_use, has_preceding_cfg_test_attr, maybe_push_export};
use super::rust_guardrails::inspect_rust_guardrail_node;
use crate::anti_cheat::populate_cst_fortress;
use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, count_children_of_kind,
    effective_param_arity, field_name_string, line_metrics, node_span, node_text,
    parse_with_language,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::types::{LanguageExtractor, ParseError};

const RUST_TUPLE_KINDS: &[&str] = &["tuple_type", "tuple_pattern"];

const RUST_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_expression",
        "while_expression",
        "for_expression",
        "loop_expression",
    ],
    flat_branches: &[],
    switch_nodes: &["match_expression"],
    case_arm_nodes: &["match_arm"],
    subtract_first_arm: true,
    bool_expr_nodes: &["binary_expression"],
    bool_ops: &["&&", "||"],
    nested_fn_nodes: &["function_item"],
    else_wrapper_nodes: &["else_clause"],
};

/// `Tree-sitter` AST extractor for Rust source files.
#[derive(Clone, Copy, Debug, Default)]
pub struct RustExtractor;

impl LanguageExtractor for RustExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_rust::LANGUAGE.into();
        let tree = parse_with_language(&ts_lang, source)?;
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Rust, total_lines, effective_lines)?;
        let root = tree.root_node();
        doc.syntax_errors = collect_syntax_errors(root);
        doc.referenced_symbols = collect_referenced_symbols(root, source);
        walk_rust(root, source, false, &mut doc);
        populate_cst_fortress(root, source, &mut doc);
        Ok(doc)
    }
}

fn count_struct_bool_fields(body: Option<Node<'_>>, source: &str) -> u32 {
    let Some(b) = body else {
        return 0;
    };
    let mut cursor = b.walk();
    let matched = b
        .named_children(&mut cursor)
        .filter(|ch| match ch.kind() {
            "field_declaration" => ch
                .child_by_field_name("type")
                .is_some_and(|t| node_text(t, source).trim() == "bool"),
            "ordered_field_declaration" => ch
                .child_by_field_name("type")
                .or_else(|| {
                    let mut fc = ch.walk();
                    ch.named_children(&mut fc).last()
                })
                .is_some_and(|t| node_text(t, source).trim() == "bool"),
            _ => false,
        })
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

fn extract_type(node: Node<'_>, source: &str) -> Option<TypeDecl> {
    let span = node_span(node);
    let body = node.child_by_field_name("body");
    let mut decl = TypeDecl {
        name: String::new(),
        kind: TypeKind::Struct,
        span,
        field_count: 0,
        bool_field_count: 0,
        method_count: 0,
    };
    match node.kind() {
        "struct_item" => {
            decl.name = field_name_string(node, "name", "<anon>", source);
            decl.field_count =
                count_children_of_kind(body, &["field_declaration", "ordered_field_declaration"]);
            decl.bool_field_count = count_struct_bool_fields(body, source);
        }
        "enum_item" => {
            decl.kind = TypeKind::Enum;
            decl.name = field_name_string(node, "name", "<anon>", source);
            decl.field_count = count_children_of_kind(body, &["enum_variant"]);
        }
        "trait_item" => {
            decl.kind = TypeKind::Interface;
            decl.name = field_name_string(node, "name", "<anon>", source);
            decl.method_count =
                count_children_of_kind(body, &["function_item", "function_signature_item"]);
        }
        "impl_item" => {
            decl.kind = TypeKind::Impl;
            decl.name = field_name_string(node, "type", "<impl>", source);
            decl.method_count = count_children_of_kind(body, &["function_item"]);
        }
        _ => return None,
    }
    Some(decl)
}

fn extract_fn(node: Node<'_>, source: &str) -> CallableDecl {
    let is_method = node
        .parent()
        .is_some_and(|p| p.kind() == "declaration_list");
    assemble_callable(
        CallableSpec {
            name: field_name_string(node, "name", "<fn>", source),
            kind: if is_method {
                CallableKind::Method
            } else {
                CallableKind::Function
            },
            decl_node: node,
            body_node: node.child_by_field_name("body"),
            param_count: effective_param_arity(
                node.child_by_field_name("parameters"),
                &["parameter"],
                RUST_TUPLE_KINDS,
            ),
        },
        source,
        &RUST_COMPLEXITY,
    )
}

fn extract_named_closure(node: Node<'_>, source: &str) -> Option<CallableDecl> {
    let val = node.child_by_field_name("value")?;
    if val.kind() != "closure_expression" {
        return None;
    }
    let body = val.child_by_field_name("body")?;
    if node_span(val).line_count() <= 1 && body.kind() != "block" {
        return None;
    }
    Some(assemble_callable(
        CallableSpec {
            name: field_name_string(node, "pattern", "<closure>", source),
            kind: CallableKind::Closure,
            decl_node: val,
            body_node: Some(body),
            param_count: effective_param_arity(
                val.child_by_field_name("parameters"),
                &["parameter", "identifier", "tuple_pattern"],
                RUST_TUPLE_KINDS,
            ),
        },
        source,
        &RUST_COMPLEXITY,
    ))
}

fn record_rust_type_node(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    let Some(ty) = extract_type(node, source) else {
        return;
    };
    if !in_cfg_test {
        maybe_push_export(node, &ty.name, source, doc);
    }
    doc.types.push(ty);
}

fn record_rust_ui_tr(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, source).trim();
    if !(ftxt.ends_with("ui_tr") || ftxt.ends_with("ui_tr_fmt")) {
        return;
    }
    let Some(args) = node.child_by_field_name("arguments") else {
        return;
    };
    let mut ac = args.walk();
    for ch in args.named_children(&mut ac) {
        if ch.kind() == "string_literal" {
            let k = node_text(ch, source).trim_matches('"');
            if !k.is_empty() {
                doc.translation_keys_used.insert(String::from(k));
                return;
            }
        }
    }
}

fn record_rust_node(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    inspect_rust_guardrail_node(node, source, in_cfg_test, doc);
    match node.kind() {
        "call_expression" => record_rust_ui_tr(node, source, doc),
        "use_declaration" => doc.imports.push(extract_use(node, source)),
        "function_item" => {
            let callable = extract_fn(node, source);
            if !in_cfg_test {
                maybe_push_export(node, &callable.name, source, doc);
            }
            doc.callables.push(callable);
        }
        "const_item" | "type_item" | "static_item" if !in_cfg_test => {
            let name = field_name_string(node, "name", "", source);
            if !name.is_empty() {
                maybe_push_export(node, &name, source, doc);
            }
        }
        "let_declaration" => {
            if let Some(closure) = extract_named_closure(node, source) {
                doc.callables.push(closure);
            }
        }
        _ => record_rust_type_node(node, source, in_cfg_test, doc),
    }
}

fn walk_rust(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    let next_in_test =
        in_cfg_test || (node.kind() == "mod_item" && has_preceding_cfg_test_attr(node, source));
    record_rust_node(node, source, next_in_test, doc);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_rust(child, source, next_in_test, doc);
    }
}
