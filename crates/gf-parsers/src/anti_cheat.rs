//! CST-only anti-bypass detector (`GF070`) and comment debt scanner (`GF074`).
//!
//! Inspects exclusively syntax-tree comment and attribute nodes (including nested
//! `#[cfg_attr(..., allow(...))]` directives) so string literals never false-positive.

mod comment_debt;
mod rust_attrs;

use gf_core::{BypassFinding, KissYagniFinding, KissYagniKind, Span, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};
use crate::kiss_contracts::extract_contracts_and_impls;
use crate::kiss_yagni::extract_kiss_and_yagni;
use crate::test_inspector_script::extract_test_cases;
use comment_debt::{
    is_rust_doc_comment, match_debt_marker, scan_children_for_commented_code, strip_comment_line,
};
use rust_attrs::inspect_rust_attribute;

const COMMENT_BYPASS_PREFIXES: &[&str] = &[
    "eslint-disable",
    "@ts-ignore",
    "@ts-nocheck",
    "biome-ignore",
    "nolint",
    "hadolint ignore",
    "shellcheck disable",
    "ruff: noqa",
    "flake8: noqa",
    "pylint: disable",
    "pragma: no cover",
    "coverage: ignore",
    "istanbul ignore",
    "c8 ignore",
    "v8 ignore",
    "type: ignore",
    "pyright: ignore",
    "mypy: ignore-errors",
    "clippy::",
    "stryker disable",
    "pragma: no mutate",
    "cosmic-ray: disable",
];

fn is_unjustified_ts_expect_error(clean: &str) -> bool {
    let Some(after) = clean.strip_prefix("@ts-expect-error") else {
        return false;
    };
    let trimmed = after.trim();
    let reason = trimmed
        .strip_prefix(':')
        .or_else(|| trimmed.strip_prefix("--"))
        .map_or("", str::trim);
    reason.chars().count() < 10 || !reason.chars().any(char::is_alphanumeric)
}

fn is_valid_prefix_tail(prefix: &str, rest: &str) -> bool {
    prefix.ends_with("::")
        || rest.is_empty()
        || rest.starts_with(|c: char| !c.is_ascii_alphanumeric() && c != '_')
}

fn match_single_bypass_segment(seg: &str, skip_doc_clippy: bool) -> Option<String> {
    let lower = seg.to_ascii_lowercase();
    for &prefix in COMMENT_BYPASS_PREFIXES {
        if prefix == "clippy::" && skip_doc_clippy {
            continue;
        }
        if lower
            .strip_prefix(prefix)
            .is_some_and(|rest| is_valid_prefix_tail(prefix, rest))
        {
            return Some(String::from(prefix));
        }
    }
    if lower == "noqa" || lower.starts_with("noqa:") || lower.starts_with("noqa ") {
        return Some(String::from("noqa"));
    }
    is_unjustified_ts_expect_error(seg)
        .then(|| String::from("@ts-expect-error (missing justification)"))
}

fn match_comment_bypass(clean: &str, skip_doc_clippy: bool) -> Option<String> {
    clean
        .split('#')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .find_map(|seg| match_single_bypass_segment(seg, skip_doc_clippy))
}

fn scan_comment_text(raw: &str, base_span: Span, doc: &mut UastDocument) {
    let is_doc = is_rust_doc_comment(raw);
    for (offset, line) in raw.lines().enumerate() {
        let clean = strip_comment_line(line);
        if clean.is_empty() {
            continue;
        }
        let line_no = base_span
            .start_line()
            .saturating_add(u32::try_from(offset).unwrap_or(0));
        let line_span = Span::new(line_no, line_no).unwrap_or(base_span);

        if let Some(directive) = match_comment_bypass(clean, is_doc) {
            doc.bypass_findings.push(BypassFinding {
                directive,
                span: line_span,
            });
        }
        if !is_doc && let Some(marker) = match_debt_marker(clean) {
            doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::TodoMarker,
                measured: 1,
                detail: format!("deferred `{marker}` comment marker"),
                span: line_span,
            });
        }
    }
}

/// Runs all CST fortress extractors (`GF070`, `GF073`, `GF074`, `GF075`, `GF076`).
pub fn populate_cst_fortress(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_anti_cheat(root, source, doc);
    extract_kiss_and_yagni(root, source, doc);
    extract_contracts_and_impls(root, source, doc);
    extract_test_cases(root, source, doc);
}

fn walk_anti_cheat(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    scan_children_for_commented_code(node, source, doc);
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
