//! PowerShell CST UAST metrics (`function_statement` callables, `param(...)` arity & bools,
//! complexity, Type-2 fingerprints) and `SuppressMessageAttribute` anti-bypass (`GF070`).

use gf_core::{
    BypassFinding, CallableKind, KissYagniFinding, KissYagniKind, UastDocument, UastKind, UastNode,
};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{CallableSpec, assemble_callable, node_span, node_text};

const POWERSHELL_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "foreach_statement",
        "for_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
    ],
    flat_branches: &["elseif_clause"],
    switch_nodes: &["switch_statement"],
    case_arm_nodes: &["switch_clause"],
    subtract_first_arm: false,
    bool_expr_nodes: &["logical_expression"],
    bool_ops: &["-and", "-or"],
    nested_fn_nodes: &["function_statement"],
    else_wrapper_nodes: &["else_clause"],
};

const PS_BOOL_TYPES: &[&str] = &["[switch]", "[bool]", "boolean]", "switchparameter]"];

fn is_ps_bool_param(param_node: Node<'_>, source: &str) -> bool {
    let raw = node_text(param_node, source);
    let lower = raw.to_ascii_lowercase();
    if PS_BOOL_TYPES.iter().any(|pat| lower.contains(pat)) {
        return true;
    }
    raw.split_once('=').is_some_and(|(_, rhs)| {
        let val = rhs.trim();
        val.eq_ignore_ascii_case("$true") || val.eq_ignore_ascii_case("$false")
    })
}

fn count_params_in_list(param_list: Node<'_>, source: &str) -> (u32, u32) {
    let mut total = 0_u32;
    let mut bools = 0_u32;
    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        if child.kind() == "script_parameter" {
            total = total.saturating_add(1);
            if is_ps_bool_param(child, source) {
                bools = bools.saturating_add(1);
            }
        }
    }
    (total, bools)
}

fn find_param_list_in_header(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "parameter_list" {
        return Some(node);
    }
    if node.kind() == "statement_list" {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_statement" {
            continue;
        }
        if let Some(found) = find_param_list_in_header(child) {
            return Some(found);
        }
    }
    None
}

fn record_ps_bool_params(node: Node<'_>, name: &str, bool_count: u32, doc: &mut UastDocument) {
    if bool_count >= 1 {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::ExcessiveBoolParams,
            measured: bool_count,
            detail: format!(
                "callable `{name}` takes {bool_count} `[switch]`/`[bool]` flag parameter(s)"
            ),
            span: node_span(node),
        });
    }
}

fn extract_ps_function(
    node: Node<'_>,
    source: &str,
    doc: &mut UastDocument,
    uast_children: &mut Vec<UastNode>,
) {
    let mut cursor = node.walk();
    let name = node
        .children(&mut cursor)
        .find(|n| n.kind() == "function_name")
        .map_or_else(
            || String::from("<fn>"),
            |n| String::from(node_text(n, source).trim()),
        );
    let mut c_body = node.walk();
    let body = node
        .children(&mut c_body)
        .find(|n| n.kind() == "script_block");
    let (param_count, bool_count) =
        find_param_list_in_header(node).map_or((0, 0), |pl| count_params_in_list(pl, source));
    record_ps_bool_params(node, &name, bool_count, doc);

    let decl = assemble_callable(
        CallableSpec {
            name: name.clone(),
            kind: CallableKind::Function,
            decl_node: node,
            body_node: body,
            param_count,
        },
        source,
        &POWERSHELL_COMPLEXITY,
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

fn has_valid_suppress_justification(attr_text: &str) -> bool {
    let lower = attr_text.to_ascii_lowercase();
    let Some(idx) = lower.find("justification") else {
        return false;
    };
    let after = attr_text[idx + "justification".len()..].trim_start();
    let Some(rhs) = after.strip_prefix('=').map(str::trim_start) else {
        return false;
    };
    let quote = rhs.chars().next();
    let Some(q @ ('"' | '\'')) = quote else {
        return false;
    };
    let Some((val, _)) = rhs[q.len_utf8()..].split_once(q) else {
        return false;
    };
    val.trim().chars().count() >= 10 && val.chars().any(char::is_alphanumeric)
}

fn check_ps_suppress_attribute(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let raw = node_text(node, source);
    if raw.to_ascii_lowercase().contains("suppressmessage")
        && !has_valid_suppress_justification(raw)
    {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from("SuppressMessageAttribute (missing Justification)"),
            span: node_span(node),
        });
    }
}

/// Walks the PowerShell CST to extract `callables`, `[switch]`/`[bool]` counts, and `SuppressMessageAttribute` bypasses.
pub fn collect_powershell_uast_metrics(
    node: Node<'_>,
    source: &str,
    doc: &mut UastDocument,
    uast_children: &mut Vec<UastNode>,
) {
    match node.kind() {
        "function_statement" => extract_ps_function(node, source, doc, uast_children),
        "attribute" => check_ps_suppress_attribute(node, source, doc),
        "command_name" => {
            let sym = node_text(node, source).trim();
            if !sym.is_empty() {
                doc.referenced_symbols.insert(String::from(sym));
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_powershell_uast_metrics(child, source, doc, uast_children);
    }
}
