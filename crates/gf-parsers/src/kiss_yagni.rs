//! CST extractors for KISS (`GF073`: nested ternaries & Law of Demeter chains)
//! and local YAGNI (`GF074`: wildcard re-exports & excessive boolean parameters).

mod demeter;

use gf_core::{KissYagniFinding, KissYagniKind, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};
use demeter::measure_outermost_property_chain;

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
            let mut c = node.walk();
            let is_pub = node
                .children(&mut c)
                .any(|ch| ch.kind() == "visibility_modifier" && node_text(ch, source) == "pub");
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
    let compact: String = raw_type
        .trim()
        .trim_start_matches(':')
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if matches!(
        compact.as_str(),
        "bool" | "boolean" | "Option<bool>" | "Optional[bool]"
    ) {
        return true;
    }
    compact.contains('|') && {
        let arms: Vec<&str> = compact.split('|').collect();
        arms.iter().any(|&a| a == "bool" || a == "boolean")
            && arms
                .iter()
                .all(|&a| matches!(a, "bool" | "boolean" | "None" | "null" | "undefined"))
    }
}

fn has_bool_default_literal(param: Node<'_>, source: &str) -> bool {
    let val_opt = param.child_by_field_name("value").or_else(|| {
        (param.kind() == "assignment_pattern")
            .then(|| param.child_by_field_name("right"))
            .flatten()
    });
    val_opt.is_some_and(|v| {
        matches!(
            node_text(v, source).trim(),
            "True" | "False" | "true" | "false"
        )
    })
}

fn count_bool_params(params_node: Option<Node<'_>>, source: &str) -> u32 {
    let Some(params) = params_node else {
        return 0;
    };
    let mut count = 0_u32;
    let mut cursor = params.walk();
    for (idx, param) in params.named_children(&mut cursor).enumerate() {
        if idx == 0
            && node_text(param, source)
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                == "this"
        {
            continue;
        }
        let typed_bool = param
            .child_by_field_name("type")
            .is_some_and(|ty| is_bool_type_text(node_text(ty, source)));
        if typed_bool || has_bool_default_literal(param, source) {
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
        "closure_expression" | "arrow_function" | "function_expression" => {
            String::from("<closure>")
        }
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
    if let Some(segments) = measure_outermost_property_chain(node, source) {
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
