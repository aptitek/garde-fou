//! Native Tier 1 PowerShell AST security & correctness rules (`PS-NULL-LEFT`, `PS-NO-IEX`,
//! `PS-PLAIN-SECRET-PARAM`, `PS-EMPTY-CATCH`, `PS-GLOBAL-VAR`).

use gf_core::{LangRuleFinding, PowerShellViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const SENSITIVE_PARAM_STEMS: &[&str] = &[
    "password",
    "passphrase",
    "secret",
    "apikey",
    "api_key",
    "accesstoken",
    "access_token",
    "privatekey",
    "private_key",
    "credential",
];

const SCALAR_AUTO_VARS: &[&str] = &["$null", "$true", "$false", "$lastexitcode", "$?", "$pid"];
const SAFE_SECRET_TYPE_MARKERS: &[&str] = &[
    "securestring",
    "pscredential",
    "[switch]",
    "[bool]",
    "boolean]",
];

const SCALAR_LIT_QUOTES: &[char] = &['\'', '"'];

fn is_scalar_constant_lhs(lhs_node: Node<'_>, source: &str) -> bool {
    let raw = node_text(lhs_node, source).trim();
    if raw.is_empty() {
        return false;
    }
    let lower = raw.to_ascii_lowercase();
    if SCALAR_AUTO_VARS.contains(&lower.as_str()) {
        return true;
    }
    if raw.len() >= 2
        && SCALAR_LIT_QUOTES
            .iter()
            .any(|&q| raw.starts_with(q) && raw.ends_with(q))
    {
        return true;
    }
    raw.parse::<f64>().is_ok_and(f64::is_finite)
}

fn check_null_comparison(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut cursor = node.walk();
    let named: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
    let [lhs, op_node, rhs] = named.as_slice() else {
        return;
    };
    let op = node_text(*op_node, source).trim();
    let is_eq_op = ["-eq", "-ne", "-ieq", "-ine", "-ceq", "-cne"]
        .iter()
        .any(|cand| op.eq_ignore_ascii_case(cand));
    let rhs_is_null = node_text(*rhs, source).trim().eq_ignore_ascii_case("$null");
    if is_eq_op && rhs_is_null && !is_scalar_constant_lhs(*lhs, source) {
        out.push(LangRuleFinding::new(
            PowerShellViolation::NullOnRight,
            node_span(node),
            format!(
                "`{}` places `$null` on the right-hand side (filters arrays instead of testing null)",
                node_text(node, source).trim()
            ),
        ));
    }
}

fn check_invoke_expression(cmd_node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut cursor = cmd_node.walk();
    let Some(target_node) = cmd_node.children(&mut cursor).find(|n| {
        n.kind() == "command_name" || (n.is_named() && n.kind() != "command_invokation_operator")
    }) else {
        return;
    };
    let raw = node_text(target_node, source)
        .trim()
        .trim_matches(['"', '\'']);
    let base = raw.rsplit('\\').next().unwrap_or(raw);
    if base.eq_ignore_ascii_case("Invoke-Expression") || base.eq_ignore_ascii_case("iex") {
        out.push(LangRuleFinding::new(
            PowerShellViolation::AvoidInvokeExpression,
            node_span(cmd_node),
            format!("`{raw}` executes arbitrary strings as PowerShell code"),
        ));
    }
}

fn check_secret_parameter(param_node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut cursor = param_node.walk();
    let Some(var_node) = param_node
        .children(&mut cursor)
        .find(|n| n.kind() == "variable")
    else {
        return;
    };
    let var_raw = node_text(var_node, source).trim();
    let var_lower = var_raw.trim_start_matches('$').to_ascii_lowercase();
    if !SENSITIVE_PARAM_STEMS.iter().any(|s| var_lower.contains(s)) {
        return;
    }
    let full_lower = node_text(param_node, source).to_ascii_lowercase();
    if !SAFE_SECRET_TYPE_MARKERS
        .iter()
        .any(|pat| full_lower.contains(pat))
    {
        out.push(LangRuleFinding::new(
            PowerShellViolation::PlainTextSecretParam,
            node_span(param_node),
            format!(
                "sensitive parameter `{var_raw}` accepts plain-text strings instead of `[SecureString]` or `[PSCredential]`"
            ),
        ));
    }
}

fn is_catch_body_empty(catch_node: Node<'_>) -> bool {
    let mut cursor = catch_node.walk();
    let Some(block) = catch_node
        .children(&mut cursor)
        .find(|n| n.kind() == "statement_block")
    else {
        return false;
    };
    let mut c2 = block.walk();
    let Some(stmt_list) = block
        .children(&mut c2)
        .find(|n| n.kind() == "statement_list")
    else {
        return true;
    };
    let mut c3 = stmt_list.walk();
    !stmt_list
        .named_children(&mut c3)
        .any(|ch| ch.kind() != "empty_statement")
}

/// Walks the PowerShell CST and emits `PowerShellViolation` findings (`GF080`).
pub fn check_powershell_rules(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "comparison_expression" | "comparison_argument_expression" => {
            check_null_comparison(node, source, out);
        }
        "command" => check_invoke_expression(node, source, out),
        "script_parameter" => check_secret_parameter(node, source, out),
        "catch_clause" if is_catch_body_empty(node) => {
            out.push(LangRuleFinding::new(
                PowerShellViolation::EmptyCatchBlock,
                node_span(node),
                "empty `catch {}` block silently swallows PowerShell exceptions",
            ));
        }
        "variable" => {
            let raw = node_text(node, source).trim();
            let lower = raw.to_ascii_lowercase();
            if lower.starts_with("$global:") || lower.starts_with("@global:") {
                out.push(LangRuleFinding::new(
                    PowerShellViolation::GlobalVariable,
                    node_span(node),
                    format!("global variable `{raw}` leaks mutable state across session scope"),
                ));
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_powershell_rules(child, source, out);
    }
}
