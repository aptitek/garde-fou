//! CST extractors for KISS (`GF073`: nested ternaries & Law of Demeter chains)
//! and local YAGNI (`GF074`: wildcard re-exports & excessive boolean parameters).

use gf_core::{KissYagniFinding, KissYagniKind, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

const FN_BOUNDARIES: &[&str] = &[
    "function_item",
    "closure_expression",
    "function_declaration",
    "method_definition",
    "arrow_function",
    "function_expression",
    "function_definition",
];

fn nested_ternary_depth(node: Node<'_>, ternary_kind: &str) -> u32 {
    let mut depth = 0_u32;
    let mut cur = node.parent();
    while let Some(p) = cur {
        if FN_BOUNDARIES.contains(&p.kind()) {
            break;
        }
        if p.kind() == ternary_kind {
            depth = depth.saturating_add(1);
        }
        cur = p.parent();
    }
    depth
}

fn chain_base_child(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("value")
        .or_else(|| node.child_by_field_name("object"))
}

fn measure_outermost_property_chain(node: Node<'_>, field_kind: &str) -> Option<u32> {
    if let Some(parent) = node.parent()
        && parent.kind() == field_kind
        && chain_base_child(parent).is_some_and(|b| b.id() == node.id())
    {
        return None;
    }
    let mut segments = 1_u32;
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind() != field_kind {
            break;
        }
        segments = segments.saturating_add(1);
        cur = chain_base_child(n);
    }
    (segments >= 3).then_some(segments)
}

fn has_descendant_kind(node: Node<'_>, target_kind: &str) -> bool {
    if node.kind() == target_kind {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| has_descendant_kind(c, target_kind))
}

fn check_wildcard_export(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let is_wildcard = match node.kind() {
        "export_statement" => {
            node.child_by_field_name("declaration").is_none()
                && (has_descendant_kind(node, "namespace_export") || {
                    let mut c = node.walk();
                    node.children(&mut c).any(|ch| ch.kind() == "*")
                })
        }
        "import_from_statement" => has_descendant_kind(node, "wildcard_import"),
        "use_declaration" => {
            let is_pub = {
                let mut c = node.walk();
                node.children(&mut c)
                    .any(|ch| ch.kind() == "visibility_modifier" && node_text(ch, source) == "pub")
            };
            is_pub
                && has_descendant_kind(node, "use_wildcard")
                && doc.path.file_name() != "prelude.rs"
        }
        _ => false,
    };
    if is_wildcard {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::WildcardReExport,
            measured: 1,
            detail: format!(
                "wildcard barrel re-export/import `{}`",
                node_text(node, source).trim()
            ),
            span: node_span(node),
        });
    }
}

fn is_bool_type_text(raw_type: &str) -> bool {
    let cleaned = raw_type
        .trim()
        .trim_start_matches(':')
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();
    cleaned == "bool" || cleaned == "boolean"
}

fn count_bool_params(params_node: Option<Node<'_>>, source: &str) -> u32 {
    let Some(params) = params_node else {
        return 0;
    };
    let mut count = 0_u32;
    let mut cursor = params.walk();
    for param in params.named_children(&mut cursor) {
        if let Some(ty_node) = param.child_by_field_name("type")
            && is_bool_type_text(node_text(ty_node, source))
        {
            count = count.saturating_add(1);
        }
    }
    count
}

fn check_callable_bool_params(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let name = match node.kind() {
        "function_item" | "function_declaration" | "method_definition" | "function_definition" => {
            field_name_string(node, "name", "<fn>", source)
        }
        "arrow_function" | "function_expression" => String::from("<closure>"),
        _ => return,
    };
    let bools = count_bool_params(node.child_by_field_name("parameters"), source);
    if bools >= 1 {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::ExcessiveBoolParams,
            measured: bools,
            detail: format!("callable `{name}` takes {bools} boolean flag parameter(s)"),
            span: node_span(node),
        });
    }
}

/// Extracts KISS (`GF073`) and local YAGNI (`GF074`) findings from a `Tree-sitter` CST.
pub fn extract_kiss_and_yagni(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_kiss(root, source, doc);
}

fn walk_kiss(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let kind = node.kind();
    if matches!(kind, "ternary_expression" | "conditional_expression") {
        let depth = nested_ternary_depth(node, kind);
        if depth >= 1 {
            doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::NestedTernary,
                measured: depth,
                detail: format!("nested ternary expression (depth {depth})"),
                span: node_span(node),
            });
        }
    }
    if matches!(kind, "field_expression" | "member_expression" | "attribute")
        && let Some(segments) = measure_outermost_property_chain(node, kind)
    {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::PropertyChain,
            measured: segments,
            detail: format!(
                "Law of Demeter property train `{}` ({segments} chained segments)",
                node_text(node, source).trim()
            ),
            span: node_span(node),
        });
    }
    check_wildcard_export(node, source, doc);
    check_callable_bool_params(node, source, doc);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_kiss(child, source, doc);
    }
}
