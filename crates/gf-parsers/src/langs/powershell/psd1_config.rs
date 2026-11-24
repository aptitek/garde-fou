//! Tier 2 CST configuration auditor (`GF081`) for `PSScriptAnalyzerSettings.psd1`.

use gf_core::{
    PsScriptAnalyzerConfigIssue, ShellInfraConfigIssue, Span, ToolConfigFinding,
    ToolConfigViolation, UastDocument,
};
use tree_sitter::Node;

use crate::cst::{collect_syntax_errors, node_span, node_text, parse_with_language};
use crate::langs::jsonc::push_cfg_violation;

const CRITICAL_PSSA_RULES: &[&str] = &[
    "psavoidusinginvokeexpression",
    "psavoidusingplaintextforpassword",
];

fn collect_psd1_strings<'a>(node: Node<'a>, source: &'a str, acc: &mut Vec<(Span, &'a str)>) {
    if matches!(
        node.kind(),
        "verbatim_string_characters" | "expandable_string_literal" | "string_literal"
    ) {
        let clean = node_text(node, source).trim().trim_matches(['"', '\'']);
        if !clean.is_empty() {
            acc.push((node_span(node), clean));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_psd1_strings(child, source, acc);
    }
}

fn check_psd1_excluded_rule(span: Span, rule: &str, out: &mut Vec<ToolConfigFinding>) {
    if rule == "*" {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::PsScriptAnalyzer(
                PsScriptAnalyzerConfigIssue::WildcardExcludeRules,
            )),
            span,
            String::from("`ExcludeRules = @('*')` disables all PSScriptAnalyzer rules"),
        );
    } else if CRITICAL_PSSA_RULES.contains(&rule.to_ascii_lowercase().as_str()) {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::PsScriptAnalyzer(
                PsScriptAnalyzerConfigIssue::CriticalSecurityRuleExcluded,
            )),
            span,
            format!("critical security rule `{rule}` excluded in `PSScriptAnalyzerSettings.psd1`"),
        );
    }
}

fn inspect_psd1_hash_entry(node: Node<'_>, source: &str, out: &mut Vec<ToolConfigFinding>) {
    let mut cursor = node.walk();
    let named: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
    let [key_node, val_node, ..] = named.as_slice() else {
        return;
    };
    let key = node_text(*key_node, source)
        .trim()
        .trim_matches(['"', '\'']);
    if !key.eq_ignore_ascii_case("ExcludeRules") {
        return;
    }
    let mut entries = Vec::new();
    collect_psd1_strings(*val_node, source, &mut entries);
    for (span, rule) in entries {
        check_psd1_excluded_rule(span, rule, out);
    }
}

fn walk_psd1_entries(node: Node<'_>, source: &str, out: &mut Vec<ToolConfigFinding>) {
    if node.kind() == "hash_entry" {
        inspect_psd1_hash_entry(node, source, out);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_psd1_entries(child, source, out);
    }
}

/// Audits `PSScriptAnalyzerSettings.psd1` via `tree-sitter-powershell` (with comment immunity).
pub fn audit_psscriptanalyzer_psd1(source: &str, doc: &mut UastDocument) {
    let ts_lang = tree_sitter_powershell::LANGUAGE.into();
    let Ok(tree) = parse_with_language(&ts_lang, source) else {
        return;
    };
    let root = tree.root_node();
    doc.syntax_errors = collect_syntax_errors(root);
    walk_psd1_entries(root, source, &mut doc.tool_config_findings);
}
