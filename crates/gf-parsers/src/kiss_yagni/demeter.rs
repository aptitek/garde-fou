//! Law of Demeter (`GF073`) property/getter navigation chain analyzer.

use tree_sitter::Node;

use crate::cst::node_text;

const NAV_FIELD_KINDS: &[&str] = &["field_expression", "member_expression", "attribute"];

const FLUENT_PIPELINE_METHODS: &[&str] = &[
    "iter",
    "into_iter",
    "iter_mut",
    "chars",
    "bytes",
    "lines",
    "split_whitespace",
    "par_iter",
    "walk",
    "peekable",
    "trim",
    "trim_start",
    "trim_end",
    "to_lowercase",
    "to_uppercase",
    "to_ascii_lowercase",
    "to_ascii_uppercase",
    "to_string",
    "to_owned",
    "into_owned",
    "as_str",
    "as_bytes",
    "as_slice",
    "as_ref",
    "as_deref",
    "as_mut",
    "cloned",
    "copied",
    "flatten",
    "collect",
    "into_values",
    "into_keys",
    "keys",
    "values",
    "enumerate",
    "ok",
    "err",
    "transpose",
    "clone",
];

fn chain_base_child(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("value")
        .or_else(|| node.child_by_field_name("object"))
}

fn member_prop_name<'a>(field_node: Node<'a>, source: &'a str) -> &'a str {
    field_node
        .child_by_field_name("field")
        .or_else(|| field_node.child_by_field_name("property"))
        .or_else(|| field_node.child_by_field_name("attribute"))
        .map_or("", |n| node_text(n, source).trim())
}

fn zero_arg_getter_base<'a>(node: Node<'a>, source: &str) -> Option<Node<'a>> {
    if !matches!(node.kind(), "call_expression" | "call") {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    if args.named_child_count() != 0 {
        return None;
    }
    let func = node.child_by_field_name("function")?;
    if !NAV_FIELD_KINDS.contains(&func.kind()) {
        return None;
    }
    let prop = member_prop_name(func, source);
    (!FLUENT_PIPELINE_METHODS.contains(&prop)).then(|| chain_base_child(func))?
}

fn is_nav_step(node: Node<'_>, source: &str) -> bool {
    NAV_FIELD_KINDS.contains(&node.kind()) || zero_arg_getter_base(node, source).is_some()
}

fn is_enclosed_in_longer_chain(node: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if NAV_FIELD_KINDS.contains(&parent.kind())
        && chain_base_child(parent).is_some_and(|b| b.id() == node.id())
    {
        return parent.parent().is_none()
            || !matches!(
                parent.parent().map(|gp| gp.kind()),
                Some("call_expression" | "call")
            )
            || zero_arg_getter_base(parent.parent().unwrap_or(parent), source).is_some();
    }
    if matches!(parent.kind(), "call_expression" | "call")
        && parent
            .child_by_field_name("function")
            .is_some_and(|f| f.id() == node.id())
    {
        return zero_arg_getter_base(parent, source).is_some();
    }
    if parent.kind() == "try_expression" {
        return parent
            .parent()
            .is_some_and(|gp| is_nav_step(gp, source) || gp.kind() == "try_expression");
    }
    false
}

/// Measures the total chained navigation segments (`a.b()?.get_c().d.e` -> `5`) for an outermost chain.
pub(super) fn measure_outermost_property_chain(node: Node<'_>, source: &str) -> Option<u32> {
    if !is_nav_step(node, source) || is_enclosed_in_longer_chain(node, source) {
        return None;
    }
    let mut segments = 1_u32;
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind() == "try_expression" {
            cur = n.named_child(0);
        } else if NAV_FIELD_KINDS.contains(&n.kind()) {
            segments = segments.saturating_add(1);
            cur = chain_base_child(n);
        } else if let Some(getter_base) = zero_arg_getter_base(n, source) {
            segments = segments.saturating_add(1);
            cur = Some(getter_base);
        } else {
            break;
        }
    }
    (segments >= 3).then_some(segments)
}
