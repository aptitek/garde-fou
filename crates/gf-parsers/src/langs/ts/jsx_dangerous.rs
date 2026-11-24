//! JSX attribute extraction, `REACT-NO-DANGEROUS-JSX`, inline `style` (`GF060`),
//! and hardcoded translatable JSX attributes (`GF062`).

use gf_core::{
    LangRuleFinding, ReactViolation, Span, StyleLeakFinding, StyleLeakKind, UastDocument,
    UiCheckFinding, UiCheckKind,
};
use tree_sitter::Node;

use super::style_and_i18n::unquote_literal;
use crate::cst::{node_span, node_text};

const TRANSLATABLE_ATTRS: &[&str] = &["placeholder", "title", "alt", "aria-label"];
const URL_ATTRS: &[&str] = &["href", "src", "formAction", "action", "xlinkHref"];

#[derive(Clone, Copy)]
pub(super) struct JsxAttr<'a> {
    pub name: &'a str,
    pub value: Option<Node<'a>>,
    pub span_node: Node<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct JsxElementRef<'a> {
    pub tag: &'a str,
    pub elem_node: Node<'a>,
    pub tag_holder: Node<'a>,
}

pub(super) fn has_two_letters(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).take(2).count() >= 2
}

fn parse_jsx_attr<'a>(attr: Node<'a>, source: &'a str) -> Option<JsxAttr<'a>> {
    if attr.kind() != "jsx_attribute" {
        return None;
    }
    let mut cursor = attr.walk();
    let mut children = attr.named_children(&mut cursor);
    let name_node = children.next()?;
    let value = children.next();
    Some(JsxAttr {
        name: node_text(name_node, source).trim(),
        value,
        span_node: attr,
    })
}

pub(super) fn collect_element_attrs<'a>(tag_node: Node<'a>, source: &'a str) -> Vec<JsxAttr<'a>> {
    let mut cursor = tag_node.walk();
    tag_node
        .named_children(&mut cursor)
        .filter_map(|child| parse_jsx_attr(child, source))
        .collect()
}

pub(super) fn static_attr_string<'a>(
    val_node: Option<Node<'a>>,
    source: &'a str,
) -> Option<&'a str> {
    let val = val_node?;
    if val.kind() == "string" {
        return unquote_literal(node_text(val, source).trim());
    }
    if val.kind() == "jsx_expression" {
        let inner = val.named_child(0)?;
        if matches!(inner.kind(), "string" | "template_string") {
            let raw = node_text(inner, source).trim();
            if !raw.contains("${") {
                return unquote_literal(raw);
            }
        }
    }
    None
}

fn is_obfuscated_javascript_uri(raw: &str) -> bool {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(char::to_lowercase)
        .take(12)
        .collect();
    cleaned.starts_with("javascript:")
}

fn check_anchor_target_blank(
    attrs: &[JsxAttr<'_>],
    tag_span: Span,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let is_blank = attrs.iter().any(|a| {
        a.name == "target"
            && static_attr_string(a.value, source).is_some_and(|v| v.eq_ignore_ascii_case("_blank"))
    });
    if !is_blank {
        return;
    }
    let safe_rel = attrs
        .iter()
        .find(|a| a.name == "rel")
        .and_then(|a| static_attr_string(a.value, source))
        .is_some_and(|r| {
            r.split_whitespace().any(|tok| {
                tok.eq_ignore_ascii_case("noopener") || tok.eq_ignore_ascii_case("noreferrer")
            })
        });
    if !safe_rel {
        out.push(LangRuleFinding::new(
            ReactViolation::DangerousJsx,
            tag_span,
            "`<a target=\"_blank\">` is missing `rel=\"noopener noreferrer\"`",
        ));
    }
}

fn inspect_single_jsx_attr(
    attr: JsxAttr<'_>,
    tag_span: Span,
    source: &str,
    doc: &mut UastDocument,
) {
    if attr.name == "dangerouslySetInnerHTML" {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            ReactViolation::DangerousJsx,
            node_span(attr.span_node),
            "`dangerouslySetInnerHTML` injects raw HTML into the DOM and risks XSS",
        ));
    } else if URL_ATTRS.contains(&attr.name)
        && static_attr_string(attr.value, source).is_some_and(is_obfuscated_javascript_uri)
    {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            ReactViolation::DangerousJsx,
            node_span(attr.span_node),
            format!("`javascript:` URI in `{}` enables DOM XSS", attr.name),
        ));
    } else if attr.name == "style" {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::InlineStyleAttr,
            detail: String::from("inline JSX `style` attribute"),
            span: tag_span,
        });
    } else if TRANSLATABLE_ATTRS.contains(&attr.name)
        && let Some(val_node) = attr.value
        && val_node.kind() == "string"
    {
        let raw = node_text(val_node, source).trim_matches(['"', '\'']);
        if has_two_letters(raw) {
            doc.ui_checks.push(UiCheckFinding {
                kind: UiCheckKind::UntranslatedText,
                detail: format!(
                    "hardcoded JSX attribute `{}=\"{raw}\"` outside `t(...)`",
                    attr.name
                ),
                span: node_span(val_node),
            });
        }
    }
}

pub(super) fn check_dangerous_jsx_and_style_attrs(
    elem: JsxElementRef<'_>,
    attrs: &[JsxAttr<'_>],
    source: &str,
    doc: &mut UastDocument,
) {
    let tag_span = node_span(elem.tag_holder);
    for &attr in attrs {
        inspect_single_jsx_attr(attr, tag_span, source, doc);
    }
    if elem.tag == "a" {
        check_anchor_target_blank(attrs, tag_span, source, &mut doc.lang_rule_findings);
    }
}
