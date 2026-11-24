//! Type-2 AST structural fingerprinting (`$ID` / `$LIT`) and symbol reference extraction.

use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::types::node_text;

const COMMENT_KINDS: &[&str] = &["comment", "line_comment", "block_comment"];

const IDENT_KINDS: &[&str] = &[
    "identifier",
    "field_identifier",
    "type_identifier",
    "property_identifier",
    "shorthand_property_identifier",
    "shorthand_property_identifier_pattern",
];

const LITERAL_KINDS: &[&str] = &[
    "integer_literal",
    "float_literal",
    "string_literal",
    "raw_string_literal",
    "char_literal",
    "boolean_literal",
    "number",
    "string",
    "integer",
    "float",
    "true",
    "false",
    "none",
    "null",
];

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

/// Computes `(Some(u128_blake3_hash), ast_node_count)` over a callable body normalized
/// for Type-2 clone detection (identifiers -> `$ID`, literals -> `$LIT`, comments stripped).
pub fn fingerprint_body(body: Node<'_>, source: &str) -> (Option<u128>, u32) {
    let mut hasher = blake3::Hasher::new();
    let mut node_count = 0_u32;
    hash_subtree(body, source, 0, (&mut hasher, &mut node_count));
    if node_count == 0 {
        return (None, 0);
    }
    let digest = hasher.finalize();
    let mut first_16 = [0_u8; 16];
    first_16.copy_from_slice(&digest.as_bytes()[..16]);
    (Some(u128::from_le_bytes(first_16)), node_count)
}

fn feed_token(hasher: &mut blake3::Hasher, depth: u16, token: &str) {
    hasher.update(&depth.to_le_bytes());
    hasher.update(token.as_bytes());
    hasher.update(&[0xFF]);
}

fn hash_subtree(node: Node<'_>, source: &str, depth: u16, state: (&mut blake3::Hasher, &mut u32)) {
    let (hasher, count) = state;
    let kind = node.kind();
    if COMMENT_KINDS.contains(&kind) {
        return;
    }

    if !node.is_named() {
        let tok = node_text(node, source).trim();
        if !tok.is_empty() {
            feed_token(hasher, depth, tok);
        }
        return;
    }

    *count = count.saturating_add(1);
    if IDENT_KINDS.contains(&kind) {
        feed_token(hasher, depth, "$ID");
        return;
    }
    if LITERAL_KINDS.contains(&kind) {
        feed_token(hasher, depth, "$LIT");
        return;
    }

    feed_token(hasher, depth, kind);
    let next_depth = depth.saturating_add(1);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        hash_subtree(child, source, next_depth, (hasher, count));
    }
}

fn is_own_declaration_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !DECL_NAME_PARENTS.contains(&parent.kind()) {
        return false;
    }
    parent
        .child_by_field_name("name")
        .is_some_and(|name_node| name_node.id() == node.id())
}

/// Collects all identifier names referenced in `root` outside their own declaration site.
pub fn collect_referenced_symbols(root: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    walk_references(root, source, &mut refs);
    refs
}

fn walk_references(node: Node<'_>, source: &str, refs: &mut BTreeSet<String>) {
    let kind = node.kind();
    if COMMENT_KINDS.contains(&kind) {
        return;
    }
    if IDENT_KINDS.contains(&kind) && !is_own_declaration_name(node) {
        let text = node_text(node, source).trim();
        if !text.is_empty() {
            refs.insert(String::from(text));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_references(child, source, refs);
    }
}
