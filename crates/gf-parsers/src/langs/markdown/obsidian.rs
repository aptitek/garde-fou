//! Obsidian Flavored Markdown (`OFM`) wikilink/embed/block-id extractor and
//! JSON Canvas 1.0 (`.canvas`) graph validator (`OBS-BROKEN-WIKILINK-OR-EMBED`,
//! `OBS-CANVAS-BROKEN-REF`).

use std::collections::BTreeSet;

use gf_core::{
    AcademicSummary, LangRuleFinding, Language, ObsidianViolation, RepoPath, Span, SyntaxErrorNode,
    UastDocument, WorkspaceRefItem, is_asset_extension,
};
use serde_json::Value;

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};

fn strip_obsidian_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut in_comment = false;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'%' && bytes[i + 1] == b'%' {
            in_comment = !in_comment;
            out.push_str("  ");
            i += 2;
            continue;
        }
        let ch = body[i..].chars().next().unwrap_or(' ');
        if in_comment && ch != '\n' {
            out.push(' ');
        } else {
            out.push(ch);
        }
        i += ch.len_utf8();
    }
    out
}

fn is_in_ranges(offset: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|&(s, e)| offset >= s && offset < e)
}

fn parse_wikilink_inner(raw: &str) -> (String, Option<String>) {
    // Strip alias separator `|` or table-escaped `\|`.
    let before_alias = raw
        .split_once("\\|")
        .map_or_else(|| raw.split_once('|').map_or(raw, |(a, _)| a), |(a, _)| a);
    if let Some((target, frag)) = before_alias.split_once('#') {
        (String::from(target.trim()), Some(String::from(frag.trim())))
    } else {
        (String::from(before_alias.trim()), None)
    }
}

struct WikilinkAcc<'a> {
    links: &'a mut Vec<WorkspaceRefItem>,
    assets: &'a mut BTreeSet<String>,
}

fn push_wikilink_match(raw_inner: &str, is_embed: bool, line: u32, acc: &mut WikilinkAcc<'_>) {
    if raw_inner.contains('\n') || raw_inner.trim().is_empty() {
        return;
    }
    let (target, fragment) = parse_wikilink_inner(raw_inner);
    let span = Span::new(line, line).unwrap_or(Span::FIRST);
    if is_embed
        && let Some((_, ext)) = target.rsplit_once('.')
        && is_asset_extension(ext)
    {
        acc.assets.insert(target.clone());
    }
    acc.links.push(WorkspaceRefItem {
        target,
        fragment,
        span,
    });
}

fn scan_wikilinks_in_body(
    clean_body: &str,
    code_ranges: &[(usize, usize)],
    body_start_line: u32,
    acc: &mut WikilinkAcc<'_>,
) {
    let bytes = clean_body.as_bytes();
    let mut i = 0;
    let mut line = body_start_line;
    while i + 3 < bytes.len() {
        if bytes[i] == b'\n' {
            line = line.saturating_add(1);
            i += 1;
            continue;
        }
        let is_open = bytes[i] == b'[' && bytes[i + 1] == b'[' && !is_in_ranges(i, code_ranges);
        if is_open && let Some(rel_end) = clean_body[i + 2..].find("]]") {
            let is_embed = i > 0 && bytes[i - 1] == b'!';
            push_wikilink_match(&clean_body[i + 2..i + 2 + rel_end], is_embed, line, acc);
            i += 4 + rel_end;
            continue;
        }
        i += 1;
    }
}

fn collect_block_ids(clean_body: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for line in clean_body.lines() {
        let trimmed = line.trim_end();
        if let Some((_, tail)) = trimmed.rsplit_once('^')
            && !tail.is_empty()
            && tail
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            let prefix_len = trimmed.len().saturating_sub(tail.len() + 1);
            if prefix_len == 0 || trimmed.as_bytes()[prefix_len - 1].is_ascii_whitespace() {
                ids.insert(String::from(tail));
            }
        }
    }
    ids
}

/// Extracts Obsidian wikilinks, embeds, and `^block-id` anchors from a Markdown body.
pub fn extract_obsidian_metadata(
    body: &str,
    code_ranges: &[(usize, usize)],
    body_start_line: u32,
    asset_refs: &mut BTreeSet<String>,
) -> Option<AcademicSummary> {
    let clean = strip_obsidian_comments(body);
    let mut wikilinks = Vec::new();
    let mut acc = WikilinkAcc {
        links: &mut wikilinks,
        assets: asset_refs,
    };
    scan_wikilinks_in_body(&clean, code_ranges, body_start_line, &mut acc);
    let block_ids = collect_block_ids(&clean);
    if wikilinks.is_empty() && block_ids.is_empty() {
        return None;
    }
    Some(AcademicSummary {
        obsidian_wikilinks: wikilinks,
        obsidian_block_ids: block_ids,
        ..AcademicSummary::default()
    })
}

fn collect_canvas_nodes(root: &Value) -> (BTreeSet<String>, Vec<WorkspaceRefItem>) {
    let mut node_ids = BTreeSet::new();
    let mut file_refs = Vec::new();
    let Some(nodes) = root.get("nodes").and_then(Value::as_array) else {
        return (node_ids, file_refs);
    };
    for node in nodes {
        if let Some(id) = node.get("id").and_then(Value::as_str) {
            node_ids.insert(String::from(id));
        }
        if node.get("type").and_then(Value::as_str) == Some("file")
            && let Some(file_path) = node.get("file").and_then(Value::as_str)
        {
            file_refs.push(WorkspaceRefItem {
                target: String::from(file_path),
                fragment: node
                    .get("subpath")
                    .and_then(Value::as_str)
                    .map(String::from),
                span: Span::FIRST,
            });
        }
    }
    (node_ids, file_refs)
}

fn audit_canvas_edges(root: &Value, node_ids: &BTreeSet<String>, out: &mut Vec<LangRuleFinding>) {
    let Some(edges) = root.get("edges").and_then(Value::as_array) else {
        return;
    };
    for edge in edges {
        for field in ["fromNode", "toNode"] {
            if let Some(target_id) = edge.get(field).and_then(Value::as_str)
                && !node_ids.contains(target_id)
            {
                out.push(LangRuleFinding::new(
                    ObsidianViolation::CanvasBrokenRef,
                    Span::FIRST,
                    format!("`.canvas` edge `{field}` references unknown node id `{target_id}`"),
                ));
            }
        }
    }
}

/// JSON Canvas 1.0 (`.canvas`) extractor validating JSON syntax, file node paths, and edge IDs.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonCanvasExtractor;

impl LanguageExtractor for JsonCanvasExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::JsonCanvas,
            total_lines,
            effective_lines,
        )?;
        let root: Value = match serde_json::from_str(source) {
            Ok(val) => val,
            Err(err) => {
                let line = u32::try_from(err.line()).unwrap_or(1).max(1);
                doc.syntax_errors.push(SyntaxErrorNode {
                    span: Span::new(line, line).unwrap_or(Span::FIRST),
                    message: format!("malformed JSON Canvas: {err}"),
                });
                return Ok(doc);
            }
        };
        let (node_ids, file_refs) = collect_canvas_nodes(&root);
        audit_canvas_edges(&root, &node_ids, &mut doc.lang_rule_findings);
        if !file_refs.is_empty() {
            doc.academic = Some(AcademicSummary {
                canvas_file_refs: file_refs,
                ..AcademicSummary::default()
            });
        }
        Ok(doc)
    }
}
