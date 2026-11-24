//! Cross-file symbol reference extractor purified against field/member homonymy (`GF021`).

use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::types::node_text;

const COMMENT_KINDS: &[&str] = &["comment", "line_comment", "block_comment"];

const DECL_NAME_PARENTS: &[&str] = &[
    "function_item",
    "struct_item",
    "enum_item",
    "trait_item",
    "type_item",
    "const_item",
    "static_item",
    "function_declaration",
    "class_declaration",
    "interface_declaration",
    "type_alias_declaration",
    "enum_declaration",
    "variable_declarator",
    "function_definition",
    "class_definition",
];

fn collect_import_subtree_idents(node: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    if matches!(node.kind(), "identifier" | "type_identifier") {
        let tok = node_text(node, source).trim();
        if !tok.is_empty() {
            out.insert(String::from(tok));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_import_subtree_idents(child, source, out);
    }
}

fn collect_imported_roots(node: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    if matches!(
        node.kind(),
        "use_declaration" | "import_statement" | "import_from_statement"
    ) {
        collect_import_subtree_idents(node, source, out);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imported_roots(child, source, out);
    }
}

fn is_own_declaration_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    DECL_NAME_PARENTS.contains(&parent.kind())
        && parent
            .child_by_field_name("name")
            .is_some_and(|name_node| name_node.id() == node.id())
}

fn is_qualified_module_or_type_base(
    base: Node<'_>,
    source: &str,
    imported_roots: &BTreeSet<String>,
) -> bool {
    let text = node_text(base, source).trim();
    let root_seg = text
        .split(['.', ':'])
        .find(|s| !s.is_empty())
        .unwrap_or(text);
    matches!(root_seg, "crate" | "super")
        || root_seg
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase())
        || imported_roots.contains(root_seg)
}

fn is_qualified_member_access(
    node: Node<'_>,
    source: &str,
    imported_roots: &BTreeSet<String>,
) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !matches!(
        parent.kind(),
        "field_expression" | "member_expression" | "attribute"
    ) {
        return false;
    }
    let prop = parent
        .child_by_field_name("field")
        .or_else(|| parent.child_by_field_name("property"))
        .or_else(|| parent.child_by_field_name("attribute"));
    if prop.is_none_or(|p| p.id() != node.id()) {
        return false;
    }
    parent
        .child_by_field_name("value")
        .or_else(|| parent.child_by_field_name("object"))
        .is_some_and(|base| is_qualified_module_or_type_base(base, source, imported_roots))
}

fn is_destructured_from_import(
    node: Node<'_>,
    source: &str,
    imported_roots: &BTreeSet<String>,
) -> bool {
    let mut cur = node.parent();
    while let Some(p) = cur {
        if p.kind() == "variable_declarator" {
            let Some(rhs) = p.child_by_field_name("value") else {
                return false;
            };
            let rhs_txt = node_text(rhs, source).trim();
            return rhs_txt.starts_with("require(") || imported_roots.contains(rhs_txt);
        }
        cur = p.parent();
    }
    false
}

fn should_record_symbol(node: Node<'_>, source: &str, imported_roots: &BTreeSet<String>) -> bool {
    if is_own_declaration_name(node) {
        return false;
    }
    match node.kind() {
        "field_identifier" | "property_identifier" => {
            is_qualified_member_access(node, source, imported_roots)
        }
        "shorthand_property_identifier_pattern" => {
            is_destructured_from_import(node, source, imported_roots)
        }
        "identifier" => {
            let is_py_attr_leaf = node.parent().is_some_and(|p| {
                p.kind() == "attribute"
                    && p.child_by_field_name("attribute")
                        .is_some_and(|a| a.id() == node.id())
            });
            if is_py_attr_leaf {
                is_qualified_member_access(node, source, imported_roots)
            } else {
                true
            }
        }
        "type_identifier" | "shorthand_property_identifier" | "shorthand_field_identifier" => true,
        _ => false,
    }
}

/// Collects identifier symbols referenced in `root` while excluding local field/property homonyms.
pub fn collect_referenced_symbols(root: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut imported_roots = BTreeSet::new();
    collect_imported_roots(root, source, &mut imported_roots);
    let mut refs = BTreeSet::new();
    walk_references(root, source, &imported_roots, &mut refs);
    refs
}

fn walk_references(
    node: Node<'_>,
    source: &str,
    imported_roots: &BTreeSet<String>,
    refs: &mut BTreeSet<String>,
) {
    if COMMENT_KINDS.contains(&node.kind()) {
        return;
    }
    if should_record_symbol(node, source, imported_roots) {
        let text = node_text(node, source).trim();
        if !text.is_empty() {
            refs.insert(String::from(text));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_references(child, source, imported_roots, refs);
    }
}
