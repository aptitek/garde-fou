//! TypeScript/JavaScript callable & class metric extraction: overload exclusion,
//! fake `this` parameter exclusion, and constructor parameter properties.

use gf_core::{CallableDecl, CallableKind, TypeDecl, TypeKind, UastDocument};
use tree_sitter::Node;

use super::routing::is_in_ambient_context;
use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, count_named_kinds, field_name_string, node_span, node_text,
    single_param_arity,
};

const TS_TUPLE_KINDS: &[&str] = &["tuple_type", "array_pattern"];

const TS_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "ternary_expression",
    ],
    flat_branches: &[],
    switch_nodes: &["switch_statement"],
    case_arm_nodes: &["switch_case"],
    subtract_first_arm: false,
    bool_expr_nodes: &["binary_expression"],
    bool_ops: &["&&", "||", "??"],
    nested_fn_nodes: &[
        "function_declaration",
        "method_definition",
        "arrow_function",
        "function_expression",
    ],
    else_wrapper_nodes: &["else_clause"],
};

fn is_ts_this_param(param: Node<'_>, source: &str) -> bool {
    node_text(param, source)
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        == "this"
}

fn is_ts_bool_annotation(node: Node<'_>, source: &str) -> bool {
    node.child_by_field_name("type")
        .is_some_and(|t| node_text(t, source).trim().trim_start_matches(':').trim() == "boolean")
}

fn count_ts_params(params_node: Option<Node<'_>>, source: &str) -> u32 {
    let Some(node) = params_node else {
        return 0;
    };
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .enumerate()
        .filter(|&(idx, c)| c.kind() != "comment" && !(idx == 0 && is_ts_this_param(c, source)))
        .map(|(_, p)| single_param_arity(p, Some(source), TS_TUPLE_KINDS))
        .fold(0_u32, u32::saturating_add)
}

fn is_constructor_param_property(param: Node<'_>) -> bool {
    if !matches!(param.kind(), "required_parameter" | "optional_parameter") {
        return false;
    }
    let mut cursor = param.walk();
    param
        .children(&mut cursor)
        .any(|ch| matches!(ch.kind(), "accessibility_modifier" | "readonly"))
}

fn tally_ctor_params(params: Node<'_>, source: &str) -> (u32, u32) {
    let mut fields = 0_u32;
    let mut bools = 0_u32;
    let mut p_cursor = params.walk();
    for param in params.named_children(&mut p_cursor) {
        if is_constructor_param_property(param) {
            fields = fields.saturating_add(1);
            if is_ts_bool_annotation(param, source) {
                bools = bools.saturating_add(1);
            }
        }
    }
    (fields, bools)
}

fn tally_constructor_param_props(body: Option<Node<'_>>, source: &str) -> (u32, u32) {
    let Some(b) = body else {
        return (0, 0);
    };
    let mut fields = 0_u32;
    let mut bools = 0_u32;
    let mut cursor = b.walk();
    for member in b.named_children(&mut cursor) {
        if member.kind() == "method_definition"
            && field_name_string(member, "name", "", source) == "constructor"
            && let Some(params) = member.child_by_field_name("parameters")
        {
            let (f, bl) = tally_ctor_params(params, source);
            fields = fields.saturating_add(f);
            bools = bools.saturating_add(bl);
        }
    }
    (fields, bools)
}

fn count_ts_bool_fields(body: Option<Node<'_>>, field_kinds: &[&str], source: &str) -> u32 {
    let Some(b) = body else {
        return 0;
    };
    let mut cursor = b.walk();
    let matched = b
        .named_children(&mut cursor)
        .filter(|ch| field_kinds.contains(&ch.kind()) && is_ts_bool_annotation(*ch, source))
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

fn extract_ts_type(node: Node<'_>, source: &str) -> Option<TypeDecl> {
    let span = node_span(node);
    let body = node.child_by_field_name("body");
    match node.kind() {
        "class_declaration" | "abstract_class_declaration" => {
            let f_kinds = &["public_field_definition", "field_definition"];
            let (ctor_fields, ctor_bools) = tally_constructor_param_props(body, source);
            let impl_methods = count_named_kinds(body, &["method_definition"]);
            Some(TypeDecl {
                name: field_name_string(node, "name", "<class>", source),
                kind: TypeKind::Class,
                span,
                field_count: count_named_kinds(body, f_kinds).saturating_add(ctor_fields),
                bool_field_count: count_ts_bool_fields(body, f_kinds, source)
                    .saturating_add(ctor_bools),
                method_count: impl_methods,
            })
        }
        "interface_declaration" => {
            let f_kinds = &["property_signature"];
            Some(TypeDecl {
                name: field_name_string(node, "name", "<interface>", source),
                kind: TypeKind::Interface,
                span,
                field_count: count_named_kinds(body, f_kinds),
                bool_field_count: count_ts_bool_fields(body, f_kinds, source),
                method_count: count_named_kinds(body, &["method_signature"]),
            })
        }
        _ => None,
    }
}

fn build_ts_callable(
    name: String,
    kind: CallableKind,
    target: Node<'_>,
    source: &str,
) -> CallableDecl {
    let param_count = target.child_by_field_name("parameter").map_or_else(
        || count_ts_params(target.child_by_field_name("parameters"), source),
        |p| single_param_arity(p, Some(source), TS_TUPLE_KINDS),
    );
    assemble_callable(
        CallableSpec {
            name,
            kind,
            decl_node: target,
            body_node: target.child_by_field_name("body"),
            param_count,
        },
        source,
        &TS_COMPLEXITY,
    )
}

/// Records executable TypeScript/JS functions, methods, and named closures while ignoring
/// bodyless overload signatures (`function_signature` / `method_signature`) and ambient `.d.ts` declarations.
pub(super) fn record_ts_callable_or_type(
    node: Node<'_>,
    source: &str,
    is_dts: bool,
    doc: &mut UastDocument,
) {
    if let Some(ty) = extract_ts_type(node, source) {
        doc.types.push(ty);
        return;
    }
    if is_in_ambient_context(node, is_dts) {
        return;
    }
    match node.kind() {
        "function_declaration" => {
            doc.callables.push(build_ts_callable(
                field_name_string(node, "name", "<fn>", source),
                CallableKind::Function,
                node,
                source,
            ));
        }
        "method_definition" => {
            doc.callables.push(build_ts_callable(
                field_name_string(node, "name", "<method>", source),
                CallableKind::Method,
                node,
                source,
            ));
        }
        "variable_declarator" => {
            if let Some(val) = node
                .child_by_field_name("value")
                .filter(|v| matches!(v.kind(), "arrow_function" | "function_expression"))
            {
                let name = field_name_string(node, "name", "<arrow>", source);
                doc.callables
                    .push(build_ts_callable(name, CallableKind::Closure, val, source));
            }
        }
        _ => {}
    }
}
