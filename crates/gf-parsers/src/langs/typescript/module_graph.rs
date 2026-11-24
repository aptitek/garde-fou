//! TypeScript/JavaScript ESM & CJS module-graph (`import`, `export ... from`,
//! dynamic `import()`, `require()`) and `console.*` effect extractor.

use gf_core::{EffectSighting, EffectSightingKind, ExportDecl, ImportDecl, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

const CONSOLE_CALLEES: &[&str] = &[
    "console.log",
    "console.error",
    "console.warn",
    "console.info",
    "console.debug",
];

fn unquote_module_specifier(raw: &str) -> String {
    String::from(raw.trim().trim_matches(['"', '\'', '`']))
}

fn collect_specifier_symbols(
    node: Node<'_>,
    specifier_kind: &str,
    source: &str,
    out: &mut Vec<String>,
) {
    if node.kind() == specifier_kind {
        let sym = field_name_string(node, "name", "", source);
        let trimmed = sym.trim();
        if !trimmed.is_empty() {
            out.push(String::from(trimmed));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_specifier_symbols(child, specifier_kind, source, out);
    }
}

/// Extracts an ESM `import_statement` along with named `import_specifier` symbols.
pub(super) fn extract_ts_import(node: Node<'_>, source: &str) -> ImportDecl {
    let module_source = node
        .child_by_field_name("source")
        .map_or_else(String::new, |s| {
            unquote_module_specifier(node_text(s, source))
        });
    let mut symbols = Vec::new();
    collect_specifier_symbols(node, "import_specifier", source, &mut symbols);
    ImportDecl {
        module_source,
        symbols,
        span: node_span(node),
    }
}

fn push_ts_export(out: &mut Vec<ExportDecl>, symbol: &str, span: gf_core::Span) {
    let trimmed = symbol.trim();
    if !trimmed.is_empty() {
        out.push(ExportDecl {
            symbol: String::from(trimmed),
            is_crate_internal: false,
            span,
        });
    }
}

fn record_ts_exported_decl(
    decl: Node<'_>,
    source: &str,
    span: gf_core::Span,
    out: &mut Vec<ExportDecl>,
) {
    if let Some(name) = decl.child_by_field_name("name") {
        push_ts_export(out, node_text(name, source), span);
        return;
    }
    if decl.kind() != "lexical_declaration" {
        return;
    }
    let mut cursor = decl.walk();
    for child in decl.named_children(&mut cursor) {
        if child.kind() == "variable_declarator"
            && let Some(name) = child.child_by_field_name("name")
        {
            push_ts_export(out, node_text(name, source), span);
        }
    }
}

/// Extracts `ExportDecl`s and barrel re-export `ImportDecl`s (`export ... from "..."`) on `export_statement`.
pub(super) fn record_ts_export_stmt(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let span = node_span(node);
    let mut reexport_symbols = Vec::new();
    collect_specifier_symbols(node, "export_specifier", source, &mut reexport_symbols);

    if let Some(src_node) = node.child_by_field_name("source") {
        let module_source = unquote_module_specifier(node_text(src_node, source));
        if !module_source.is_empty() {
            doc.imports.push(ImportDecl {
                module_source,
                symbols: reexport_symbols.clone(),
                span,
            });
        }
    }
    for sym in &reexport_symbols {
        push_ts_export(&mut doc.exports, sym, span);
    }
    if let Some(decl) = node.child_by_field_name("declaration") {
        record_ts_exported_decl(decl, source, span, &mut doc.exports);
    }
}

/// Inspects `call_expression` for dynamic `import("./x")`, `require("./x")`, and `console.*`.
pub(super) fn record_ts_call_expr(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let callee: String = node_text(func, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let span = node_span(node);

    if CONSOLE_CALLEES.contains(&callee.as_str()) {
        doc.effect_sightings.push(EffectSighting {
            kind: EffectSightingKind::ConsoleOutput,
            detail: callee,
            span,
        });
        return;
    }

    if !matches!(callee.as_str(), "import" | "require") {
        return;
    }
    let Some(args) = node.child_by_field_name("arguments") else {
        return;
    };
    let mut cursor = args.walk();
    let Some(first_arg) = args.named_children(&mut cursor).next() else {
        return;
    };
    let raw_arg = node_text(first_arg, source).trim();
    if matches!(first_arg.kind(), "string" | "template_string") && !raw_arg.contains("${") {
        let module_source = unquote_module_specifier(raw_arg);
        if !module_source.is_empty() {
            doc.imports.push(ImportDecl {
                module_source,
                symbols: Vec::new(),
                span,
            });
        }
    }
}
