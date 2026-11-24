//! Python module import (`import` / `from ... import ...`) and public export
//! (`__all__` vs top-level non-underscore definitions) extractor (`GF021`, `GF071`, `GF072`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{ExportDecl, ImportDecl, RepoPath, Span};
use tree_sitter::Node;

use super::python_typing::has_pytest_fixture_decorator;
use crate::cst::{field_name_string, node_span, node_text};

fn imported_item_symbol(item: Node<'_>, source: &str) -> String {
    if item.kind() == "aliased_import" {
        field_name_string(item, "name", "", source)
    } else {
        String::from(node_text(item, source).trim())
    }
}

/// Extracts structured [`ImportDecl`]s from Python `import_statement` or `import_from_statement`.
pub fn extract_py_imports(node: Node<'_>, source: &str, out: &mut Vec<ImportDecl>) {
    let span = node_span(node);
    if node.kind() == "import_from_statement" {
        let module_source = field_name_string(node, "module_name", node_text(node, source), source);
        let mut cursor = node.walk();
        let symbols: Vec<String> = node
            .children_by_field_name("name", &mut cursor)
            .map(|item| imported_item_symbol(item, source))
            .filter(|s| !s.is_empty())
            .collect();
        out.push(ImportDecl {
            module_source,
            symbols,
            span,
        });
        return;
    }

    let mut cursor = node.walk();
    let names: Vec<String> = node
        .children_by_field_name("name", &mut cursor)
        .map(|item| imported_item_symbol(item, source))
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        out.push(ImportDecl {
            module_source: String::from(
                node_text(node, source).trim_start_matches("import ").trim(),
            ),
            symbols: Vec::new(),
            span,
        });
    } else {
        for module_source in names {
            out.push(ImportDecl {
                module_source,
                symbols: Vec::new(),
                span,
            });
        }
    }
}

fn is_py_test_module(path: &RepoPath) -> bool {
    let fname = path.file_name();
    fname == "conftest.py" || fname.starts_with("test_") || fname.ends_with("_test.py")
}

fn unquote_py_string_literal<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if node.kind() != "string" {
        return None;
    }
    let mut cursor = node.walk();
    if let Some(content) = node
        .named_children(&mut cursor)
        .find(|c| c.kind() == "string_content")
    {
        let txt = node_text(content, source).trim();
        return (!txt.is_empty()).then_some(txt);
    }
    let trimmed = node_text(node, source)
        .trim()
        .trim_start_matches(['r', 'u', 'b', 'R', 'U', 'B'])
        .trim_matches(['"', '\'']);
    (!trimmed.is_empty()).then_some(trimmed)
}

fn collect_dunder_all_items(rhs: Node<'_>, source: &str, out: &mut Vec<(String, Span)>) {
    if !matches!(rhs.kind(), "list" | "tuple" | "set") {
        return;
    }
    let mut cursor = rhs.walk();
    for elem in rhs.named_children(&mut cursor) {
        if let Some(sym) = unquote_py_string_literal(elem, source) {
            out.push((String::from(sym), node_span(elem)));
        }
    }
}

fn extract_static_dunder_all(root: Node<'_>, source: &str) -> Option<Vec<(String, Span)>> {
    let mut found_dunder_all = false;
    let mut items = Vec::new();
    let mut cursor = root.walk();
    for stmt in root.named_children(&mut cursor) {
        if stmt.kind() != "expression_statement" {
            continue;
        }
        let Some(assign) = stmt
            .named_child(0)
            .filter(|n| matches!(n.kind(), "assignment" | "augmented_assignment"))
        else {
            continue;
        };
        let is_all = assign
            .child_by_field_name("left")
            .is_some_and(|lhs| node_text(lhs, source).trim() == "__all__");
        if is_all {
            found_dunder_all = true;
            if let Some(rhs) = assign.child_by_field_name("right") {
                collect_dunder_all_items(rhs, source, &mut items);
            }
        }
    }
    found_dunder_all.then_some(items)
}

fn unwrap_top_level_def(stmt: Node<'_>) -> Option<Node<'_>> {
    match stmt.kind() {
        "function_definition" | "class_definition" => Some(stmt),
        "decorated_definition" => stmt.child_by_field_name("definition"),
        _ => None,
    }
}

/// Extracts Python module [`ExportDecl`]s, honoring static `__all__ = [...]` and skipping test files.
pub fn extract_py_exports(root: Node<'_>, path: &RepoPath, source: &str) -> Vec<ExportDecl> {
    if is_py_test_module(path) {
        return Vec::new();
    }

    let mut top_defs: Vec<(String, Span)> = Vec::new();
    let mut def_spans: BTreeMap<String, Span> = BTreeMap::new();
    let mut cursor = root.walk();
    for stmt in root.named_children(&mut cursor) {
        let Some(def_node) = unwrap_top_level_def(stmt) else {
            continue;
        };
        if def_node.kind() == "function_definition"
            && has_pytest_fixture_decorator(def_node, source)
        {
            continue;
        }
        let name = field_name_string(def_node, "name", "", source);
        if !name.is_empty() {
            let span = node_span(def_node);
            def_spans.entry(name.clone()).or_insert(span);
            top_defs.push((name, span));
        }
    }

    if let Some(dunder_all) = extract_static_dunder_all(root, source) {
        let mut seen = BTreeSet::new();
        return dunder_all
            .into_iter()
            .filter(|(sym, _)| seen.insert(sym.clone()))
            .map(|(symbol, lit_span)| ExportDecl {
                span: def_spans.get(&symbol).copied().unwrap_or(lit_span),
                symbol,
                is_crate_internal: false,
            })
            .collect();
    }

    top_defs
        .into_iter()
        .filter(|(name, _)| !name.starts_with('_'))
        .map(|(symbol, span)| ExportDecl {
            symbol,
            is_crate_internal: false,
            span,
        })
        .collect()
}
