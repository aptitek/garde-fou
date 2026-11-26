//! Pure slice-based HTML5 & Astro template tag scanner building [`HtmlElementNode`] trees.

use gf_core::Span;

use super::aria_table::HtmlElementNode;
use super::attr_scanner::{parse_raw_tag_attrs, split_tag_head};
use super::content_model::VOID_ELEMENTS;
use crate::langs::toml_lang::span_from_byte_offset;

fn close_matching_frame(stack: &mut Vec<HtmlElementNode<'_>>, close_tag: &str) {
    let Some(pos) = stack
        .iter()
        .rposition(|n| n.tag.eq_ignore_ascii_case(close_tag))
    else {
        return;
    };
    for _ in pos..stack.len() {
        if let Some(closed) = stack.pop()
            && let Some(parent) = stack.last_mut()
        {
            parent.has_any_child_content = true;
            parent.children.push(closed);
        }
    }
}

#[derive(Clone, Copy)]
struct TagTok<'a> {
    inner: &'a str,
    span: Span,
    rest_after_gt: &'a str,
}

fn skip_raw_text_tag<'a>(
    rest_after_gt: &'a str,
    node: HtmlElementNode<'a>,
    stack: &mut [HtmlElementNode<'a>],
) -> &'a str {
    let close_pat = format!("</{}>", node.tag);
    let after_raw = rest_after_gt
        .split_once(&close_pat)
        .map_or(rest_after_gt, |(_, tail)| tail);
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
    }
    after_raw
}

fn is_implicitly_closed_void(tag: &str, tail_source: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
        && !tail_source
            .to_ascii_lowercase()
            .contains(&format!("</{tag}>"))
}

fn push_or_open_element<'a>(
    node: HtmlElementNode<'a>,
    self_closing: bool,
    rest_after_gt: &'a str,
    stack: &mut Vec<HtmlElementNode<'a>>,
) -> &'a str {
    if matches!(node.tag, "script" | "style") && !self_closing {
        return skip_raw_text_tag(rest_after_gt, node, stack);
    }
    if self_closing {
        if let Some(top) = stack.last_mut() {
            top.has_any_child_content = true;
            top.children.push(node);
        }
    } else {
        stack.push(node);
    }
    rest_after_gt
}

fn handle_tag_token<'a>(tok: TagTok<'a>, stack: &mut Vec<HtmlElementNode<'a>>) -> &'a str {
    if let Some(close_rest) = tok.inner.strip_prefix('/') {
        let close_name = close_rest.split_whitespace().next().unwrap_or("");
        if !close_name.is_empty() {
            close_matching_frame(stack, close_name);
        }
        return tok.rest_after_gt;
    }
    let raw_self_closing = tok.inner.ends_with('/');
    let clean_inner = tok.inner.strip_suffix('/').unwrap_or(tok.inner);
    let name_end = clean_inner
        .find(|c: char| c.is_ascii_whitespace() || c == '/')
        .unwrap_or(clean_inner.len());
    let tag = &clean_inner[..name_end];
    if !tag.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()) {
        return tok.rest_after_gt;
    }
    let self_closing = raw_self_closing || is_implicitly_closed_void(tag, tok.rest_after_gt);
    let node = HtmlElementNode {
        tag,
        attrs: parse_raw_tag_attrs(&clean_inner[name_end..]),
        span: tok.span,
        has_any_child_content: false,
        children: Vec::new(),
    };
    push_or_open_element(node, self_closing, tok.rest_after_gt, stack)
}

fn step_html_cursor<'a>(
    full_source: &'a str,
    rest: &'a str,
    stack: &mut Vec<HtmlElementNode<'a>>,
) -> Option<&'a str> {
    if !rest.starts_with('<') {
        let (text, after_text) = rest
            .find('<')
            .map_or((rest, ""), |idx| (&rest[..idx], &rest[idx..]));
        if !text.trim().is_empty()
            && let Some(top) = stack.last_mut()
        {
            top.has_any_child_content = true;
        }
        return Some(after_text);
    }
    if let Some(after_open) = rest.strip_prefix("<!--") {
        return Some(after_open.split_once("-->").map_or("", |(_, t)| t));
    }
    let byte_offset = full_source.len() - rest.len();
    let after_lt = rest.strip_prefix('<')?;
    let (inner, rest_after_gt) = split_tag_head(after_lt)?;
    let tok = TagTok {
        inner,
        span: span_from_byte_offset(full_source, byte_offset),
        rest_after_gt,
    };
    Some(handle_tag_token(tok, stack))
}

/// Scans an HTML5 or `.astro` template body into a forest of [`HtmlElementNode`]s.
#[must_use]
pub(in crate::langs) fn parse_html_template_tree(source: &str) -> Vec<HtmlElementNode<'_>> {
    let mut stack: Vec<HtmlElementNode<'_>> = vec![HtmlElementNode {
        tag: "__root__",
        attrs: Vec::new(),
        span: Span::FIRST,
        has_any_child_content: false,
        children: Vec::new(),
    }];
    let mut rest = source;
    while !rest.is_empty() {
        let Some(next_rest) = step_html_cursor(source, rest, &mut stack) else {
            break;
        };
        if next_rest.len() >= rest.len() {
            break;
        }
        rest = next_rest;
    }
    for _ in 1..stack.len() {
        if let Some(child) = stack.pop()
            && let Some(parent) = stack.last_mut()
        {
            parent.children.push(child);
        }
    }
    stack.pop().map_or_else(Vec::new, |root| root.children)
}
