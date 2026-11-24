//! Shell CST UAST metrics (`function_definition` callables, complexity, Type-2 hashes, `source`/`.` imports).

use std::collections::BTreeSet;

use gf_core::{CallableKind, ImportDecl, UastDocument, UastKind, UastNode};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{CallableSpec, assemble_callable, field_name_string, node_span, node_text};

const SHELL_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "for_statement",
        "c_style_for_statement",
        "while_statement",
        "until_statement",
    ],
    flat_branches: &["elif_clause"],
    switch_nodes: &["case_statement"],
    case_arm_nodes: &["case_item", "fallthrough_case_item"],
    subtract_first_arm: true,
    bool_expr_nodes: &["list", "binary_expression"],
    bool_ops: &["&&", "||"],
    nested_fn_nodes: &["function_definition"],
    else_wrapper_nodes: &["else_clause"],
};

fn parse_positional_index(raw: &str) -> Option<u32> {
    let digits = raw
        .strip_prefix("${")
        .and_then(|s| s.strip_suffix('}'))
        .or_else(|| raw.strip_prefix('$'))?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let idx = digits.parse::<u32>().ok()?;
    (idx >= 1).then_some(idx)
}

fn collect_positional_params(node: Node<'_>, source: &str, seen: &mut BTreeSet<u32>) {
    if node.kind() == "function_definition" {
        return;
    }
    if matches!(node.kind(), "simple_expansion" | "expansion")
        && let Some(idx) = parse_positional_index(node_text(node, source))
    {
        seen.insert(idx);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_positional_params(child, source, seen);
    }
}

fn count_shell_fn_params(body: Option<Node<'_>>, source: &str) -> u32 {
    let Some(body_node) = body else {
        return 0;
    };
    let mut seen = BTreeSet::new();
    collect_positional_params(body_node, source, &mut seen);
    u32::try_from(seen.len()).unwrap_or(u32::MAX)
}

fn strip_shell_quotes(raw: &str) -> &str {
    raw.trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            raw.trim()
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })
        .unwrap_or_else(|| raw.trim())
}

fn extract_source_import(cmd_node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(name_node) = cmd_node.child_by_field_name("name") else {
        return;
    };
    let cmd_name = node_text(name_node, source).trim();
    if cmd_name != "source" && cmd_name != "." {
        return;
    }
    let mut cursor = cmd_node.walk();
    let first_arg = cmd_node
        .children_by_field_name("argument", &mut cursor)
        .next();
    let Some(arg_node) = first_arg else {
        return;
    };
    let clean = strip_shell_quotes(node_text(arg_node, source));
    if !clean.is_empty() && !clean.contains('$') {
        doc.imports.push(ImportDecl {
            module_source: String::from(clean),
            symbols: Vec::new(),
            is_type_only: false,
            span: node_span(cmd_node),
        });
    }
}

/// Walks the Shell CST to populate `callables`, `imports`, `referenced_symbols`, and `root.children`.
pub fn collect_shell_uast_metrics(
    node: Node<'_>,
    source: &str,
    doc: &mut UastDocument,
    uast_children: &mut Vec<UastNode>,
) {
    match node.kind() {
        "function_definition" => {
            let name = field_name_string(node, "name", "<fn>", source);
            let body = node.child_by_field_name("body");
            let param_count = count_shell_fn_params(body, source);
            let decl = assemble_callable(
                CallableSpec {
                    name: name.clone(),
                    kind: CallableKind::Function,
                    decl_node: node,
                    body_node: body,
                    param_count,
                },
                source,
                &SHELL_COMPLEXITY,
            );
            uast_children.push(UastNode::leaf(
                UastKind::Function {
                    name,
                    param_count: decl.param_count,
                    cognitive_complexity: decl.cognitive_complexity,
                },
                decl.span,
            ));
            doc.callables.push(decl);
        }
        "command" => {
            extract_source_import(node, source, doc);
            if let Some(name_node) = node.child_by_field_name("name") {
                let sym = node_text(name_node, source).trim();
                if !sym.is_empty() {
                    doc.referenced_symbols.insert(String::from(sym));
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_shell_uast_metrics(child, source, doc, uast_children);
    }
}
