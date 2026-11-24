//! CST-only anti-bypass detector (`GF070`) and comment debt scanner (`GF074`).
//!
//! Inspects exclusively syntax-tree comment and attribute nodes so string literals
//! inside parser/test source files never trigger false positives.

use gf_core::{BypassFinding, KissYagniFinding, KissYagniKind, Span, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};
use crate::kiss_contracts::extract_contracts_and_impls;
use crate::kiss_yagni::extract_kiss_and_yagni;
use crate::test_inspector::extract_test_cases;

const COMMENT_BYPASS_PREFIXES: &[&str] = &[
    "eslint-disable",
    "@ts-ignore",
    "@ts-nocheck",
    "biome-ignore",
    "nolint",
    "hadolint ignore",
    "shellcheck disable",
    "ruff: noqa",
    "pylint: disable",
    "pragma: no cover",
    "type: ignore",
];

const DEBT_MARKERS: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];

fn is_rust_doc_comment(raw: &str) -> bool {
    let t = raw.trim_start();
    t.starts_with("///") || t.starts_with("//!") || t.starts_with("/**") || t.starts_with("/*!")
}

fn strip_comment_line(line: &str) -> &str {
    let t = line.trim();
    let without_open = t
        .strip_prefix("//")
        .or_else(|| t.strip_prefix("/*"))
        .or_else(|| t.strip_prefix('#'))
        .or_else(|| t.strip_prefix('*'))
        .unwrap_or(t);
    without_open
        .strip_suffix("*/")
        .unwrap_or(without_open)
        .trim()
}

fn is_unjustified_ts_expect_error(clean: &str) -> bool {
    let Some(after) = clean.strip_prefix("@ts-expect-error") else {
        return false;
    };
    let trimmed = after.trim();
    let reason = trimmed
        .strip_prefix(':')
        .or_else(|| trimmed.strip_prefix("--"))
        .map_or("", str::trim);
    reason.chars().filter(|c| c.is_alphanumeric()).count() < 3
}

fn match_comment_bypass(clean: &str) -> Option<String> {
    let lower = clean.to_ascii_lowercase();
    for &prefix in COMMENT_BYPASS_PREFIXES {
        if let Some(rest) = lower.strip_prefix(prefix)
            && (rest.is_empty()
                || rest.starts_with(|c: char| !c.is_ascii_alphanumeric() && c != '_'))
        {
            return Some(String::from(prefix));
        }
    }
    if lower == "noqa" || lower.starts_with("noqa:") || lower.starts_with("noqa ") {
        return Some(String::from("noqa"));
    }
    is_unjustified_ts_expect_error(clean)
        .then(|| String::from("@ts-expect-error (missing justification)"))
}

fn match_debt_marker(clean: &str) -> Option<&'static str> {
    for tok in clean.split_whitespace() {
        let word = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if let Some(&marker) = DEBT_MARKERS.iter().find(|&&m| word == m) {
            return Some(marker);
        }
        for &marker in DEBT_MARKERS {
            if tok.len() > marker.len()
                && tok[..marker.len()].eq_ignore_ascii_case(marker)
                && matches!(tok.as_bytes().get(marker.len()), Some(b':' | b'('))
            {
                return Some(marker);
            }
        }
    }
    None
}

fn scan_comment_text(raw: &str, base_span: Span, doc: &mut UastDocument) {
    let skip_debt = is_rust_doc_comment(raw);
    for (offset, line) in raw.lines().enumerate() {
        let clean = strip_comment_line(line);
        if clean.is_empty() {
            continue;
        }
        let line_no = base_span
            .start_line()
            .saturating_add(u32::try_from(offset).unwrap_or(0));
        let line_span = Span::new(line_no, line_no).unwrap_or(base_span);

        if let Some(directive) = match_comment_bypass(clean) {
            doc.bypass_findings.push(BypassFinding {
                directive,
                span: line_span,
            });
        }
        if !skip_debt && let Some(marker) = match_debt_marker(clean) {
            doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::TodoMarker,
                measured: 1,
                detail: format!("deferred `{marker}` comment marker"),
                span: line_span,
            });
        }
    }
}

fn has_non_empty_reason_arg(attr_inner: &str) -> bool {
    let Some((_, after_reason)) = attr_inner.split_once("reason") else {
        return false;
    };
    let eq_rhs = after_reason.trim_start();
    let Some(val_part) = eq_rhs.strip_prefix('=') else {
        return false;
    };
    let trimmed = val_part.trim_start();
    let Some(after_quote) = trimmed.strip_prefix('"') else {
        return false;
    };
    after_quote
        .split_once('"')
        .is_some_and(|(reason_body, _)| reason_body.trim().len() >= 3)
}

fn inspect_rust_attribute(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let raw = node_text(node, source).trim();
    let inner = raw
        .trim_start_matches("#!")
        .trim_start_matches('#')
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .map_or("", str::trim);
    let compact: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.starts_with("allow(") {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from(concat!("#[", "allow(...)]")),
            span: node_span(node),
        });
    } else if compact.starts_with("expect(") && !has_non_empty_reason_arg(inner) {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from(concat!(
                "#[",
                "expect(...)] without non-empty `reason = \"...\"`"
            )),
            span: node_span(node),
        });
    }
}

/// Runs all Step 7 CST fortress extractors (`GF070`, `GF073`, `GF074`, `GF075`, `GF076`).
pub fn populate_cst_fortress(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    extract_cst_anti_cheat(root, source, doc);
    extract_kiss_and_yagni(root, source, doc);
    extract_contracts_and_impls(root, source, doc);
    extract_test_cases(root, source, doc);
}

fn extract_cst_anti_cheat(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_anti_cheat(root, source, doc);
}

fn walk_anti_cheat(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "comment" | "line_comment" | "block_comment" => {
            scan_comment_text(node_text(node, source), node_span(node), doc);
        }
        "attribute_item" | "inner_attribute_item" => {
            inspect_rust_attribute(node, source, doc);
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_anti_cheat(child, source, doc);
    }
}

/// Scans `#` comment lines in `Dockerfile` or `Shell` sources for `hadolint`/`shellcheck` bypasses.
pub fn scan_hash_line_comments(source: &str, doc: &mut UastDocument) {
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') || trimmed.starts_with("#!") {
            continue;
        }
        let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
        let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
        scan_comment_text(trimmed, span, doc);
    }
}
