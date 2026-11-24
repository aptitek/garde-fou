//! TypeScript/JavaScript ESM & CJS module graph (`ImportDecl`, `ExportDecl`) with exact
//! `import type` / inline `{ type X }` / `export type` separation and declaration-merging deduplication.

use std::collections::BTreeMap;

use gf_core::{ExportDecl, ImportDecl, Span, UastDocument};
use tree_sitter::Node;

use super::routing::is_in_ambient_context;
use crate::cst::{field_name_string, node_span, node_text};

fn unquote_module_specifier(raw: &str) -> String {
    String::from(raw.trim().trim_matches(['"', '\'', '`']))
}

fn has_unnamed_type_modifier(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|ch| !ch.is_named() && matches!(ch.kind(), "type" | "typeof"))
}

#[derive(Default)]
struct ImportSpecScan {
    symbols: Vec<String>,
    has_value_binding: bool,
    spec_count: usize,
    value_spec_count: usize,
}

fn scan_import_clause(node: Node<'_>, source: &str, scan: &mut ImportSpecScan) {
    match node.kind() {
        "import_specifier" => {
            scan.spec_count = scan.spec_count.saturating_add(1);
            if !has_unnamed_type_modifier(node) {
                scan.value_spec_count = scan.value_spec_count.saturating_add(1);
            }
            let sym = field_name_string(node, "name", "", source);
            if !sym.trim().is_empty() {
                scan.symbols.push(String::from(sym.trim()));
            }
            return;
        }
        "namespace_import" => {
            scan.has_value_binding = true;
            return;
        }
        "identifier" => {
            scan.has_value_binding = true;
            scan.symbols
                .push(String::from(node_text(node, source).trim()));
            return;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        scan_import_clause(child, source, scan);
    }
}

/// Extracts an ESM `import_statement` with exact `is_type_only` classification.
pub(super) fn extract_ts_import(node: Node<'_>, source: &str) -> ImportDecl {
    let module_source = node
        .child_by_field_name("source")
        .map_or_else(String::new, |s| {
            unquote_module_specifier(node_text(s, source))
        });
    let mut cursor = node.walk();
    let clause = node
        .named_children(&mut cursor)
        .find(|c| c.kind() == "import_clause");
    let top_level_type = has_unnamed_type_modifier(node);
    let mut scan = ImportSpecScan::default();
    if let Some(cl) = clause {
        scan_import_clause(cl, source, &mut scan);
    }
    let all_specs_type_only =
        !scan.has_value_binding && scan.spec_count > 0 && scan.value_spec_count == 0;
    ImportDecl {
        module_source,
        symbols: scan.symbols,
        is_type_only: top_level_type || all_specs_type_only,
        span: node_span(node),
    }
}

fn collect_export_specifiers(
    node: Node<'_>,
    stmt_is_type: bool,
    source: &str,
    out: &mut Vec<(String, String, bool)>,
) {
    if node.kind() == "export_specifier" {
        let orig = field_name_string(node, "name", "", source);
        let exported = field_name_string(node, "alias", &orig, source);
        let is_type = stmt_is_type || has_unnamed_type_modifier(node);
        if !orig.trim().is_empty() {
            out.push((
                String::from(orig.trim()),
                String::from(exported.trim()),
                is_type,
            ));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_export_specifiers(child, stmt_is_type, source, out);
    }
}

fn push_ts_export(out: &mut Vec<ExportDecl>, symbol: &str, is_type_only: bool, span: Span) {
    let trimmed = symbol.trim();
    if !trimmed.is_empty() {
        out.push(ExportDecl {
            symbol: String::from(trimmed),
            is_crate_internal: false,
            is_type_only,
            span,
        });
    }
}

fn record_exported_decl(decl: Node<'_>, source: &str, span: Span, out: &mut Vec<ExportDecl>) {
    if decl.kind() == "function_signature" {
        return;
    }
    let is_type_decl = matches!(
        decl.kind(),
        "type_alias_declaration" | "interface_declaration"
    );
    if let Some(name) = decl.child_by_field_name("name") {
        push_ts_export(out, node_text(name, source), is_type_decl, span);
        return;
    }
    if !matches!(decl.kind(), "lexical_declaration" | "variable_declaration") {
        return;
    }
    let mut cursor = decl.walk();
    for child in decl.named_children(&mut cursor) {
        if child.kind() == "variable_declarator"
            && let Some(name) = child.child_by_field_name("name")
            && name.kind() == "identifier"
        {
            push_ts_export(out, node_text(name, source), false, span);
        }
    }
}

fn record_ambient_reexport(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if let Some(src_node) = node.child_by_field_name("source") {
        let module_source = unquote_module_specifier(node_text(src_node, source));
        if !module_source.is_empty() {
            doc.imports.push(ImportDecl {
                module_source,
                symbols: Vec::new(),
                is_type_only: true,
                span: node_span(node),
            });
        }
    }
}

/// Extracts `ExportDecl`s and re-export `ImportDecl`s on `export_statement`.
pub(super) fn record_ts_export_stmt(
    node: Node<'_>,
    source: &str,
    is_dts: bool,
    doc: &mut UastDocument,
) {
    if is_in_ambient_context(node, is_dts) {
        record_ambient_reexport(node, source, doc);
        return;
    }
    let span = node_span(node);
    let stmt_is_type = has_unnamed_type_modifier(node);
    let mut specs = Vec::new();
    collect_export_specifiers(node, stmt_is_type, source, &mut specs);

    if let Some(src_node) = node.child_by_field_name("source") {
        let module_source = unquote_module_specifier(node_text(src_node, source));
        if !module_source.is_empty() {
            let reexport_type_only =
                stmt_is_type || (!specs.is_empty() && specs.iter().all(|(_, _, is_t)| *is_t));
            doc.imports.push(ImportDecl {
                module_source,
                symbols: specs.iter().map(|(orig, _, _)| orig.clone()).collect(),
                is_type_only: reexport_type_only,
                span,
            });
        }
    }
    for (_, exported, is_t) in &specs {
        push_ts_export(&mut doc.exports, exported, *is_t, span);
    }
    if let Some(decl) = node.child_by_field_name("declaration") {
        record_exported_decl(decl, source, span, &mut doc.exports);
    }
}

/// Inspects `call_expression` for dynamic `import("./x")` and `require("./x")`.
pub(super) fn record_dynamic_import_call(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let callee = node_text(func, source).trim();
    if !matches!(callee, "import" | "require") {
        return;
    }
    let Some(first_arg) = node
        .child_by_field_name("arguments")
        .and_then(|a| a.named_child(0))
    else {
        return;
    };
    let raw_arg = node_text(first_arg, source).trim();
    if matches!(first_arg.kind(), "string" | "template_string") && !raw_arg.contains("${") {
        let module_source = unquote_module_specifier(raw_arg);
        if !module_source.is_empty() {
            doc.imports.push(ImportDecl {
                module_source,
                symbols: Vec::new(),
                is_type_only: false,
                span: node_span(node),
            });
        }
    }
}

/// Deduplicates TypeScript declaration-merged exports (`export type Brand` + `export const Brand`)
/// in a single module, keeping `is_type_only = false` whenever a value binding exists.
pub(super) fn merge_declaration_exports(exports: &mut Vec<ExportDecl>) {
    let mut merged: BTreeMap<String, usize> = BTreeMap::new();
    let mut deduped: Vec<ExportDecl> = Vec::with_capacity(exports.len());
    for exp in exports.drain(..) {
        if let Some(&idx) = merged.get(&exp.symbol) {
            if !exp.is_type_only {
                deduped[idx].is_type_only = false;
            }
        } else {
            merged.insert(exp.symbol.clone(), deduped.len());
            deduped.push(exp);
        }
    }
    *exports = deduped;
}
