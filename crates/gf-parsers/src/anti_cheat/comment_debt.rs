//! Comment debt (`TODO`/`FIXME`) and `CommentedOutCode` (`GF074`) CST scanners.

use gf_core::{KissYagniFinding, KissYagniKind, Span, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const DEBT_MARKERS: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];

const CODE_LINE_PREFIXES: &[&str] = &[
    "fn ",
    "pub fn ",
    "let ",
    "const ",
    "if ",
    "for ",
    "return ",
    "def ",
    "import ",
    "export function ",
];

pub fn is_rust_doc_comment(raw: &str) -> bool {
    let t = raw.trim_start();
    t.starts_with("///") || t.starts_with("//!") || t.starts_with("/**") || t.starts_with("/*!")
}

pub fn strip_comment_line(line: &str) -> &str {
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

fn match_prefixed_debt_token(tok: &str) -> Option<&'static str> {
    DEBT_MARKERS.iter().copied().find(|marker| {
        tok.as_bytes()
            .get(..marker.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(marker.as_bytes()))
            && matches!(tok.as_bytes().get(marker.len()), Some(b':' | b'('))
    })
}

pub fn match_debt_marker(clean: &str) -> Option<&'static str> {
    for tok in clean.split_whitespace() {
        let word = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if let Some(&marker) = DEBT_MARKERS.iter().find(|&&m| word == m) {
            return Some(marker);
        }
        if let Some(marker) = match_prefixed_debt_token(tok) {
            return Some(marker);
        }
    }
    None
}

fn is_license_or_url_line(clean: &str) -> bool {
    let lower = clean.to_ascii_lowercase();
    lower.contains("spdx-license-identifier")
        || lower.contains("copyright")
        || lower.contains("licensed under")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
}

fn is_code_like_comment_line(clean: &str) -> bool {
    if clean.is_empty() || is_license_or_url_line(clean) {
        return false;
    }
    if CODE_LINE_PREFIXES.iter().any(|pfx| clean.starts_with(pfx)) {
        return true;
    }
    !clean.starts_with('-')
        && !clean.starts_with('*')
        && !clean.starts_with('#')
        && (clean.ends_with(';') || clean.ends_with('{') || clean.ends_with('}'))
}

fn evaluate_comment_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    span: Span,
    doc: &mut UastDocument,
) {
    let mut total_lines = 0_u32;
    let mut code_markers = 0_u32;
    for raw_line in lines {
        total_lines = total_lines.saturating_add(1);
        if is_code_like_comment_line(strip_comment_line(raw_line)) {
            code_markers = code_markers.saturating_add(1);
        }
    }
    if total_lines >= 3 && code_markers >= 2 {
        doc.kiss_yagni_findings.push(KissYagniFinding {
            kind: KissYagniKind::CommentedOutCode,
            measured: total_lines,
            detail: format!("commented-out dead code block ({total_lines} lines)"),
            span,
        });
    }
}

fn is_multiline_comment(node: Node<'_>, source: &str) -> bool {
    node.kind() == "block_comment"
        || (node.kind() == "comment" && node_text(node, source).contains('\n'))
}

fn is_non_doc_line_comment(node: Node<'_>, source: &str) -> bool {
    let raw = node_text(node, source);
    matches!(node.kind(), "comment" | "line_comment")
        && !raw.contains('\n')
        && !is_rust_doc_comment(raw)
}

fn find_comment_run_end(children: &[Node<'_>], start_idx: usize, source: &str) -> (usize, u32) {
    let mut end_idx = start_idx;
    let mut end_line = node_span(children[start_idx]).end_line();
    while let Some(&next) = children.get(end_idx + 1) {
        let next_span = node_span(next);
        if is_non_doc_line_comment(next, source) && next_span.start_line() == end_line + 1 {
            end_line = next_span.end_line();
            end_idx += 1;
        } else {
            break;
        }
    }
    (end_idx, end_line)
}

/// Scans `parent`'s direct children to detect `>= 3` consecutive `line_comment`s of dead code,
/// as well as multi-line `block_comment`s containing dead code (`GF074`).
pub fn scan_children_for_commented_code(parent: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut cursor = parent.walk();
    let children: Vec<Node<'_>> = parent.children(&mut cursor).collect();
    let mut idx = 0;
    while idx < children.len() {
        let child = children[idx];
        if is_multiline_comment(child, source) {
            let raw = node_text(child, source);
            if !is_rust_doc_comment(raw) {
                evaluate_comment_lines(raw.lines(), node_span(child), doc);
            }
            idx += 1;
        } else if is_non_doc_line_comment(child, source) {
            let start_span = node_span(child);
            let (end_idx, end_line) = find_comment_run_end(&children, idx, source);
            if end_idx >= idx + 2 {
                let span = Span::new(start_span.start_line(), end_line).unwrap_or(start_span);
                let lines = children[idx..=end_idx]
                    .iter()
                    .map(|n| node_text(*n, source));
                evaluate_comment_lines(lines, span, doc);
            }
            idx = end_idx + 1;
        } else {
            idx += 1;
        }
    }
}
