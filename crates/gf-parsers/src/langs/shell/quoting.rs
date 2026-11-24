//! Contextual word-splitting analysis (`SH-SC2086` & `SH-SC2046`) faithful to `ShellCheck`'s
//! `AnalyzerLib.hs` (`isQuoteFreeNode`).

use gf_core::{LangRuleFinding, ShellViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const SAFE_SPECIAL_EXPANSIONS: &[&str] = &[
    "$?", "$#", "$$", "$!", "$-", "$LINENO", "$OPTIND", "$PPID", "$UID", "$EUID", "$RANDOM",
    "$SRANDOM", "$SECONDS", "$SHLVL", "$BASHPID",
];

fn is_non_splitting_var(node: Node<'_>, source: &str) -> bool {
    let raw = node_text(node, source).trim();
    match node.kind() {
        "simple_expansion" => SAFE_SPECIAL_EXPANSIONS.contains(&raw),
        "expansion" => raw.starts_with("${#"),
        _ => false,
    }
}

fn is_double_bracket_test(parent: Node<'_>, source: &str) -> bool {
    node_text(parent, source).trim_start().starts_with("[[")
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum QuoteVerdict {
    QuoteFree,
    SplitsWords,
    ContinueUp,
}

fn classify_parent_boundary(child: Node<'_>, parent: Node<'_>, source: &str) -> QuoteVerdict {
    match parent.kind() {
        "string"
        | "translated_string"
        | "heredoc_body"
        | "ansi_c_string"
        | "raw_string"
        | "arithmetic_expansion"
        | "compound_statement"
        | "c_style_for_statement"
        | "subscript"
        | "case_item"
        | "fallthrough_case_item" => QuoteVerdict::QuoteFree,
        "test_command" => {
            if is_double_bracket_test(parent, source) {
                QuoteVerdict::QuoteFree
            } else {
                QuoteVerdict::SplitsWords
            }
        }
        "variable_assignment" => {
            let is_lhs = parent.child_by_field_name("name") == Some(child);
            if is_lhs {
                QuoteVerdict::SplitsWords
            } else {
                QuoteVerdict::QuoteFree
            }
        }
        "case_statement" => {
            if parent.child_by_field_name("value") == Some(child) {
                QuoteVerdict::QuoteFree
            } else {
                QuoteVerdict::ContinueUp
            }
        }
        "command"
        | "for_statement"
        | "array"
        | "command_substitution"
        | "subshell"
        | "process_substitution"
        | "file_redirect"
        | "herestring_redirect"
        | "program" => QuoteVerdict::SplitsWords,
        _ => QuoteVerdict::ContinueUp,
    }
}

fn is_quote_free(node: Node<'_>, source: &str) -> bool {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        match classify_parent_boundary(cur, parent, source) {
            QuoteVerdict::QuoteFree => return true,
            QuoteVerdict::SplitsWords => return false,
            QuoteVerdict::ContinueUp => cur = parent,
        }
    }
    false
}

fn has_enclosing_unquoted_expansion(node: Node<'_>, source: &str) -> bool {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if matches!(
            parent.kind(),
            "string" | "command_substitution" | "command" | "for_statement" | "test_command"
        ) {
            return false;
        }
        if parent.kind() == "expansion" && !is_quote_free(parent, source) {
            return true;
        }
        cur = parent;
    }
    false
}

/// Walks the Shell CST and appends `SH-SC2086` / `SH-SC2046` findings for unquoted expansions.
pub fn check_unquoted_expansions(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "simple_expansion" | "expansion"
            if !is_non_splitting_var(node, source)
                && !is_quote_free(node, source)
                && !has_enclosing_unquoted_expansion(node, source) =>
        {
            out.push(LangRuleFinding::new(
                ShellViolation::UnquotedExpansion,
                node_span(node),
                format!(
                    "unquoted expansion `{}` is subject to word-splitting and globbing",
                    node_text(node, source).trim()
                ),
            ));
        }
        "command_substitution"
            if !is_quote_free(node, source) && !has_enclosing_unquoted_expansion(node, source) =>
        {
            out.push(LangRuleFinding::new(
                ShellViolation::UnquotedCommandSub,
                node_span(node),
                format!(
                    "unquoted command substitution `{}` splits output on `IFS`",
                    node_text(node, source).trim()
                ),
            ));
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_unquoted_expansions(child, source, out);
    }
}
