//! Unified W3C HTML5 content-model & WAI-ARIA 1.2 / WCAG 2.2 AA/AAA analyzer
//! for `.html`, `.astro`, and `.tsx`/`.jsx` (`GF066`, `GF080`).

mod a11y_controls;
mod aria_table;
mod attr_scanner;
mod content_model;
mod html_scanner;

use gf_core::{LangRuleFinding, Language, RepoPath, UastDocument, UiCheckFinding, UiCheckKind};
use tree_sitter::Node;

use crate::cst::{line_metrics, node_span, node_text};
use crate::types::{LanguageExtractor, ParseError};
pub(super) use aria_table::{HtmlAttrView, HtmlElementNode};
pub(super) use attr_scanner::parse_raw_tag_attrs;
pub(super) use content_model::audit_html_element_tree;
pub(super) use html_scanner::parse_html_template_tree;

fn unquote_attr_lit(s: &str) -> Option<&str> {
    s.strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
}

fn extract_jsx_expr_static(raw: &str) -> Option<&str> {
    let s = raw.strip_prefix('{')?.strip_suffix('}')?.trim();
    match s {
        "true" | "false" => Some(s),
        _ if s.parse::<i64>().is_ok() => Some(s),
        _ => unquote_attr_lit(s),
    }
}

fn jsx_attr_to_view<'a>(attr_node: Node<'a>, source: &'a str) -> Option<HtmlAttrView<'a>> {
    if attr_node.kind() != "jsx_attribute" {
        return None;
    }
    let name_node = attr_node.child(0)?;
    let name = node_text(name_node, source).trim();
    let Some(val_node) = attr_node.child(2) else {
        return Some(HtmlAttrView {
            name,
            static_val: None,
            is_dynamic: false,
        });
    };
    let raw = node_text(val_node, source).trim();
    match val_node.kind() {
        "string" => Some(HtmlAttrView {
            name,
            static_val: unquote_attr_lit(raw),
            is_dynamic: false,
        }),
        "jsx_expression" => {
            let static_lit = extract_jsx_expr_static(raw);
            Some(HtmlAttrView {
                name,
                static_val: static_lit,
                is_dynamic: static_lit.is_none(),
            })
        }
        _ => Some(HtmlAttrView {
            name,
            static_val: Some(raw),
            is_dynamic: false,
        }),
    }
}

fn build_jsx_element_node<'a>(node: Node<'a>, source: &'a str) -> Option<HtmlElementNode<'a>> {
    let header = if node.kind() == "jsx_element" {
        node.child_by_field_name("open_tag")?
    } else if node.kind() == "jsx_self_closing_element" {
        node
    } else {
        return None;
    };
    let name_node = header.child_by_field_name("name")?;
    let tag = node_text(name_node, source).trim();
    let mut attrs = Vec::new();
    let mut c = header.walk();
    for ch in header.named_children(&mut c) {
        if let Some(view) = jsx_attr_to_view(ch, source) {
            attrs.push(view);
        }
    }
    let mut children = Vec::new();
    let mut has_any_child_content = false;
    if node.kind() == "jsx_element" {
        collect_jsx_children(node, source, &mut has_any_child_content, &mut children);
    }
    Some(HtmlElementNode {
        tag,
        attrs,
        span: node_span(header),
        has_any_child_content,
        children,
    })
}

fn collect_jsx_children<'a>(
    parent: Node<'a>,
    source: &'a str,
    has_content: &mut bool,
    out: &mut Vec<HtmlElementNode<'a>>,
) {
    let mut cursor = parent.walk();
    for ch in parent.named_children(&mut cursor) {
        match ch.kind() {
            "jsx_opening_element" | "jsx_closing_element" => {}
            "jsx_text" => {
                if !node_text(ch, source).trim().is_empty() {
                    *has_content = true;
                }
            }
            _ => {
                *has_content = true;
                collect_jsx_expr_children(ch, source, out);
            }
        }
    }
}

fn collect_jsx_expr_children<'a>(
    node: Node<'a>,
    source: &'a str,
    out: &mut Vec<HtmlElementNode<'a>>,
) {
    if matches!(
        node.kind(),
        "lexical_declaration" | "variable_declaration" | "function_declaration"
    ) {
        return;
    }
    if matches!(node.kind(), "jsx_element" | "jsx_self_closing_element") {
        if let Some(elem) = build_jsx_element_node(node, source) {
            out.push(elem);
        }
        return;
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        collect_jsx_expr_children(ch, source, out);
    }
}

fn collect_nested_decl_jsx_roots<'a>(
    node: Node<'a>,
    source: &'a str,
    out: &mut Vec<HtmlElementNode<'a>>,
) {
    if matches!(
        node.kind(),
        "lexical_declaration" | "variable_declaration" | "function_declaration"
    ) {
        collect_jsx_roots(node, source, out);
        return;
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        collect_nested_decl_jsx_roots(ch, source, out);
    }
}

fn collect_jsx_roots<'a>(node: Node<'a>, source: &'a str, out: &mut Vec<HtmlElementNode<'a>>) {
    if matches!(node.kind(), "jsx_element" | "jsx_self_closing_element") {
        if let Some(elem) = build_jsx_element_node(node, source) {
            out.push(elem);
        }
        collect_nested_decl_jsx_roots(node, source, out);
        return;
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        collect_jsx_roots(ch, source, out);
    }
}

/// Audits all JSX/TSX elements in `root` against W3C HTML5 content models and WCAG 2.2 AA (`GF066`/`GF080`).
pub(crate) fn audit_jsx_tree_w3c_a11y(
    root: Node<'_>,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut roots = Vec::new();
    collect_jsx_roots(root, source, &mut roots);
    audit_html_element_tree(&roots, out);
}

fn check_html_img_and_button_ui(nodes: &[HtmlElementNode<'_>], out: &mut Vec<UiCheckFinding>) {
    for n in nodes {
        if n.tag == "img" && !n.attrs.iter().any(|a| a.name.eq_ignore_ascii_case("alt")) {
            out.push(UiCheckFinding {
                kind: UiCheckKind::ImgMissingAlt,
                detail: String::from("`<img>` element is missing required `alt` attribute"),
                span: n.span,
            });
        }
        if n.tag == "button" && !n.attrs.iter().any(|a| a.name.eq_ignore_ascii_case("type")) {
            out.push(UiCheckFinding {
                kind: UiCheckKind::ButtonMissingType,
                detail: String::from(
                    "`<button>` missing explicit `type` (`button` | `submit` | `reset`)",
                ),
                span: n.span,
            });
        }
        check_html_img_and_button_ui(&n.children, out);
    }
}

/// Specification-grade W3C HTML5 & WCAG 2.2 AA extractor for `.html` and `.htm` documents.
#[derive(Clone, Copy, Debug, Default)]
pub struct HtmlExtractor;

impl LanguageExtractor for HtmlExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Html, total_lines, effective_lines)?;
        let tree = parse_html_template_tree(source);
        audit_html_element_tree(&tree, &mut doc.lang_rule_findings);
        check_html_img_and_button_ui(&tree, &mut doc.ui_checks);
        Ok(doc)
    }
}
