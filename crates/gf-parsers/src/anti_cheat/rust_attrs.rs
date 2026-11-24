//! Recursive Rust attribute scanner (`#[allow]`, `#[warn]`, `#[cfg_attr(..., allow)]`, `#[expect]`).

use std::collections::BTreeSet;

use gf_core::{BypassFinding, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

fn has_justified_expect_reason(expect_args: &str) -> bool {
    let Some((_, after_reason)) = expect_args.split_once("reason") else {
        return false;
    };
    let Some(val_part) = after_reason.trim_start().strip_prefix('=') else {
        return false;
    };
    let Some(after_quote) = val_part.trim_start().strip_prefix('"') else {
        return false;
    };
    let Some((reason_body, _)) = after_quote.split_once('"') else {
        return false;
    };
    let alpha_chars = reason_body.chars().filter(|c| c.is_alphabetic()).count();
    let distinct_words: BTreeSet<String> = reason_body
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    alpha_chars >= 12 && distinct_words.len() >= 2
}

const fn advance_string_state(ch: char, prev_esc: &mut bool) -> bool {
    if ch == '"' && !*prev_esc {
        *prev_esc = false;
        return false;
    }
    *prev_esc = ch == '\\' && !*prev_esc;
    true
}

fn extract_balanced_parens(after_open: &str) -> &str {
    let mut depth = 1_u32;
    let mut in_str = false;
    let mut prev_esc = false;
    for (idx, ch) in after_open.char_indices() {
        if in_str {
            in_str = advance_string_state(ch, &mut prev_esc);
        } else if ch == '"' {
            in_str = true;
        } else if ch == '(' {
            depth = depth.saturating_add(1);
        } else if ch == ')' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return after_open.get(..idx).unwrap_or("");
            }
        }
    }
    after_open
}

fn classify_attr_directive(short_ident: &str, args: &str) -> Option<&'static str> {
    match short_ident {
        "allow" => Some("rust allow(...) lint bypass"),
        "warn" => Some("rust warn(...) lint downgrade"),
        "expect" if !has_justified_expect_reason(args) => {
            Some("rust expect(...) without justified `reason` (>= 12 alpha chars, >= 2 words)")
        }
        _ => None,
    }
}

fn emit_ident_call(raw: &str, start: usize, paren_idx: usize) -> (&str, &str) {
    let full_ident = raw.get(start..paren_idx).unwrap_or("");
    let short = full_ident.rsplit("::").next().unwrap_or(full_ident);
    let args = extract_balanced_parens(raw.get(paren_idx + 1..).unwrap_or(""));
    (short, args)
}

fn collect_attr_calls(raw: &str) -> Vec<(&str, &str)> {
    let mut calls = Vec::new();
    let mut in_str = false;
    let mut prev_esc = false;
    let mut ident_start: Option<usize> = None;

    for (idx, ch) in raw.char_indices() {
        if in_str {
            in_str = advance_string_state(ch, &mut prev_esc);
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                ident_start = None;
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == ':' => {
                ident_start = Some(ident_start.unwrap_or(idx));
            }
            '(' => {
                if let Some(start) = ident_start.take() {
                    calls.push(emit_ident_call(raw, start, idx));
                }
            }
            c if !c.is_whitespace() => ident_start = None,
            _ => {}
        }
    }
    calls
}

fn has_mutants_skip_token(raw: &str) -> bool {
    let mut unquoted = String::with_capacity(raw.len());
    let mut in_str = false;
    let mut prev_esc = false;
    for ch in raw.chars() {
        if in_str {
            in_str = advance_string_state(ch, &mut prev_esc);
        } else if ch == '"' {
            in_str = true;
            unquoted.push(' ');
        } else if !ch.is_whitespace() {
            unquoted.push(ch);
        }
    }
    unquoted.contains("mutants::skip")
}

/// Inspects a Rust `#[...]` or `#![...]` attribute node, including nested `cfg_attr(...)` directives.
pub(super) fn inspect_rust_attribute(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let raw = node_text(node, source).trim();
    let span = node_span(node);
    if has_mutants_skip_token(raw) {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from("#[mutants::skip]"),
            span,
        });
    }
    for (short_ident, args) in collect_attr_calls(raw) {
        if let Some(directive) = classify_attr_directive(short_ident, args) {
            doc.bypass_findings.push(BypassFinding {
                directive: String::from(directive),
                span,
            });
        }
    }
}
