//! Deployed parameter arity unpacking (`GF003` anti-tuple-packing).

use tree_sitter::Node;

use crate::types::node_text;

const TRANSPARENT_ARITY_WRAPPERS: &[&str] = &[
    "reference_type",
    "pointer_type",
    "type_annotation",
    "type",
    "parenthesized_pattern",
    "parenthesized_type",
    "mut_pattern",
    "ref_pattern",
];

fn unwrap_arity_node(mut node: Node<'_>) -> Node<'_> {
    while TRANSPARENT_ARITY_WRAPPERS.contains(&node.kind()) {
        let mut cursor = node.walk();
        let Some(inner) = node
            .named_children(&mut cursor)
            .find(|c| c.kind() != "mutable_specifier" && c.kind() != "lifetime")
        else {
            break;
        };
        node = inner;
    }
    node
}

fn node_deployed_arity(node: Node<'_>, source: Option<&str>, tuple_kinds: &[&str]) -> u32 {
    let cur = unwrap_arity_node(node);
    if tuple_kinds.contains(&cur.kind()) {
        let mut cursor = cur.walk();
        let sum: u32 = cur
            .named_children(&mut cursor)
            .filter(|c| c.kind() != "comment")
            .map(|el| node_deployed_arity(el, source, tuple_kinds))
            .fold(0_u32, u32::saturating_add);
        return sum.max(1);
    }
    if cur.kind() == "generic_type"
        && let Some(src) = source
        && cur
            .named_child(0)
            .is_some_and(|id| matches!(node_text(id, src).trim(), "tuple" | "Tuple"))
        && let Some(type_params) = cur.named_child(1)
    {
        let mut cursor = type_params.walk();
        let sum: u32 = type_params
            .named_children(&mut cursor)
            .filter(|c| c.kind() != "comment")
            .map(|el| node_deployed_arity(el, source, tuple_kinds))
            .fold(0_u32, u32::saturating_add);
        return sum.max(1);
    }
    1
}

/// Computes the deployed arity of a single parameter node, recursively unpacking tuples (`GF003`).
pub fn single_param_arity(param: Node<'_>, source: Option<&str>, tuple_kinds: &[&str]) -> u32 {
    let direct = node_deployed_arity(param, source, tuple_kinds);
    let pat = param
        .child_by_field_name("pattern")
        .or_else(|| param.child_by_field_name("name"))
        .map_or(1, |n| node_deployed_arity(n, source, tuple_kinds));
    let ty = param
        .child_by_field_name("type")
        .map_or(1, |n| node_deployed_arity(n, source, tuple_kinds));
    direct.max(pat).max(ty).max(1)
}

/// Computes total deployed parameter arity across `params_node`, unpacking tuple-packed params (`GF003`).
pub fn effective_param_arity(
    params_node: Option<Node<'_>>,
    param_kinds: &[&str],
    tuple_kinds: &[&str],
) -> u32 {
    let Some(node) = params_node else {
        return 0;
    };
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| param_kinds.contains(&c.kind()))
        .map(|param| single_param_arity(param, None, tuple_kinds))
        .fold(0_u32, u32::saturating_add)
}
