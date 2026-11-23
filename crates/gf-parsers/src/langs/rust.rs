//! Safe `Tree-sitter` extractor for Rust (`.rs`).

use gf_core::{
    CallableDecl, CallableKind, ExportDecl, ImportDecl, Language, RepoPath, TypeDecl, TypeKind,
    UastDocument, UastKind, UastNode,
};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, count_children_of_kind,
    field_name_string, line_metrics, node_span, node_text, parse_with_language,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::{LanguageExtractor, ParseError};

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
        walk_rust(root, source, &mut doc);
        Ok(doc)
    }
}

fn is_top_level_public(node: Node<'_>, source: &str) -> bool {
    if node.parent().is_none_or(|p| p.kind() != "source_file") {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| c.kind() == "visibility_modifier" && node_text(c, source) == "pub")
}

fn extract_use(node: Node<'_>, source: &str) -> ImportDecl {
    let raw = node
        .child_by_field_name("argument")
        .map_or_else(|| node_text(node, source), |arg| node_text(arg, source))
        .trim();
    let symbols = raw
        .split_once('{')
        .and_then(|(_, rest)| rest.rsplit_once('}'))
        .map_or_else(Vec::new, |(inner, _)| {
            inner
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        });
    ImportDecl {
        module_source: String::from(raw),
        symbols,
        span: node_span(node),
    }
}

fn extract_type(node: Node<'_>, source: &str) -> Option<TypeDecl> {
    let span = node_span(node);
    let body = node.child_by_field_name("body");
    match node.kind() {
        "struct_item" => Some(TypeDecl {
            name: field_name_string(node, "name", "<anon>", source),
            kind: TypeKind::Struct,
            span,
            field_count: count_children_of_kind(
                body,
                &["field_declaration", "ordered_field_declaration"],
            ),
            method_count: 0,
        }),
        "enum_item" => Some(TypeDecl {
            name: field_name_string(node, "name", "<anon>", source),
            kind: TypeKind::Enum,
            span,
            field_count: count_children_of_kind(body, &["enum_variant"]),
            method_count: 0,
        }),
        "trait_item" => Some(TypeDecl {
            name: field_name_string(node, "name", "<anon>", source),
            kind: TypeKind::Interface,
            span,
            field_count: 0,
            method_count: count_children_of_kind(
                body,
                &["function_item", "function_signature_item"],
            ),
        }),
        "impl_item" => Some(TypeDecl {
            name: field_name_string(node, "type", "<impl>", source),
            kind: TypeKind::Impl,
            span,
            field_count: 0,
            method_count: count_children_of_kind(body, &["function_item"]),
        }),
        _ => None,
    }
}

fn extract_fn(node: Node<'_>, source: &str) -> CallableDecl {
    let is_method = node
        .parent()
        .is_some_and(|p| p.kind() == "declaration_list");
    let kind = if is_method {
        CallableKind::Method
    } else {
        CallableKind::Function
    };
    assemble_callable(
        CallableSpec {
            name: field_name_string(node, "name", "<fn>", source),
            kind,
            decl_node: node,
            body_node: node.child_by_field_name("body"),
            param_count: count_children_of_kind(
                node.child_by_field_name("parameters"),
                &["parameter"],
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
            param_count: count_children_of_kind(
                val.child_by_field_name("parameters"),
                &["parameter", "identifier"],
            ),
        },
        source,
        &RUST_COMPLEXITY,
    ))
}

fn maybe_push_export(node: Node<'_>, symbol: &str, source: &str, doc: &mut UastDocument) {
    if is_top_level_public(node, source) {
        doc.exports.push(ExportDecl {
            symbol: String::from(symbol),
            span: node_span(node),
        });
    }
}

fn record_rust_node(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "use_declaration" => doc.imports.push(extract_use(node, source)),
        "function_item" => {
            let callable = extract_fn(node, source);
            maybe_push_export(node, &callable.name, source, doc);
            doc.callables.push(callable);
        }
        "const_item" | "type_item" | "static_item" => {
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
        "unsafe_block" => doc.root.children.push(UastNode::leaf(
            UastKind::ForbiddenCall {
                symbol: String::from("unsafe"),
            },
            node_span(node),
        )),
        _ => {
            if let Some(ty) = extract_type(node, source) {
                maybe_push_export(node, &ty.name, source, doc);
                doc.types.push(ty);
            }
        }
    }
}

fn walk_rust(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    record_rust_node(node, source, doc);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_rust(child, source, doc);
    }
}
