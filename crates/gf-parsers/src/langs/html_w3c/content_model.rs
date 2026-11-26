//! WHATWG / W3C HTML5 content models and WCAG 2.2 AA element tree evaluator
//! (`W3C-*` & `A11Y-*` — `GF066`/`GF080`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, W3cViolation};

use super::a11y_controls::{
    attr_static, check_focus_and_tabindex, check_media_and_form_control, has_attr, is_hidden_input,
};
use super::aria_table::{HtmlAttrView, HtmlElementNode, validate_element_aria};

#[rustfmt::skip]
pub(super) const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img",
    "input", "link", "meta", "source", "track", "wbr",
];

#[rustfmt::skip]
const OBSOLETE_ELEMENTS: &[&str] = &[
    "acronym", "applet", "big", "blink", "center", "dir",
    "font", "frame", "frameset", "marquee", "noframes", "strike", "tt",
];

#[rustfmt::skip]
const BLOCK_ELEMENTS: &[&str] = &[
    "article", "aside", "blockquote", "div", "dl", "fieldset", "footer", "form",
    "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "main", "nav", "ol",
    "p", "pre", "section", "table", "ul",
];

fn is_intrinsic_tag(tag: &str) -> bool {
    tag.bytes().next().is_some_and(|b| b.is_ascii_lowercase()) && !tag.contains(':')
}

fn is_interactive_control(tag: &str, attrs: &[HtmlAttrView<'_>]) -> bool {
    match tag {
        "button" | "select" | "textarea" | "details" | "embed" | "iframe" => true,
        "a" => has_attr(attrs, "href"),
        "input" => !is_hidden_input(tag, attrs),
        _ => false,
    }
}

const ANC_INTERACTIVE: u8 = 1;
const ANC_FORM: u8 = 2;
const ANC_P_OR_SPAN: u8 = 4;
const ANC_LABEL: u8 = 8;
const ANC_ARIA_HIDDEN: u8 = 16;

#[derive(Clone, Copy, Default)]
struct AncestorCtx<'a> {
    parent_tag: Option<&'a str>,
    flags: u8,
}

impl AncestorCtx<'_> {
    const fn has(self, mask: u8) -> bool {
        (self.flags & mask) != 0
    }
}

fn violates_parent_child_model(parent: Option<&str>, child_tag: &str) -> bool {
    match parent {
        Some("ul" | "ol") => !matches!(child_tag, "li" | "script" | "template"),
        Some("tr") => !matches!(child_tag, "td" | "th" | "script" | "template"),
        Some("dl") => !matches!(child_tag, "dt" | "dd" | "div" | "script" | "template"),
        _ => false,
    }
}

fn is_nested_interactive_or_form(node: &HtmlElementNode<'_>, anc: AncestorCtx<'_>) -> bool {
    (anc.has(ANC_INTERACTIVE) && is_interactive_control(node.tag, &node.attrs))
        || (anc.has(ANC_FORM) && node.tag == "form")
}

#[rustfmt::skip]
fn check_w3c_content_rules(node: &HtmlElementNode<'_>, anc: AncestorCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    if VOID_ELEMENTS.contains(&node.tag) && node.has_any_child_content {
        out.push(LangRuleFinding::new(
            W3cViolation::VoidElementChildren,
            node.span,
            format!("HTML5 void element `<{}>` must not have children", node.tag),
        ));
    }
    if OBSOLETE_ELEMENTS.contains(&node.tag) {
        out.push(LangRuleFinding::new(
            W3cViolation::ContentModel,
            node.span,
            format!("obsolete HTML tag `<{}>` is forbidden in W3C HTML5", node.tag),
        ));
    }
    if is_nested_interactive_or_form(node, anc) {
        out.push(LangRuleFinding::new(
            W3cViolation::InteractiveNesting,
            node.span,
            format!("interactive `<{}>` must not be nested inside another interactive control or `<form>`", node.tag),
        ));
    }
    if anc.has(ANC_P_OR_SPAN) && BLOCK_ELEMENTS.contains(&node.tag) {
        out.push(LangRuleFinding::new(
            W3cViolation::ContentModel,
            node.span,
            format!("block element `<{}>` is forbidden inside `<p>` or `<span>`", node.tag),
        ));
    }
    if violates_parent_child_model(anc.parent_tag, node.tag) {
        out.push(LangRuleFinding::new(
            W3cViolation::ContentModel,
            node.span,
            format!("`<{}>` is not a permitted direct child of `<{}>`", node.tag, anc.parent_tag.unwrap_or("")),
        ));
    }
}

struct DocAuditState {
    seen_ids: BTreeSet<String>,
    labeled_ids: BTreeSet<String>,
    visible_main_count: u32,
    total_nav_count: u32,
    total_aside_count: u32,
}

fn collect_doc_landmarks_and_labels(nodes: &[HtmlElementNode<'_>], state: &mut DocAuditState) {
    for n in nodes {
        if n.tag == "nav" {
            state.total_nav_count += 1;
        } else if n.tag == "aside" {
            state.total_aside_count += 1;
        } else if n.tag == "label"
            && let Some(target) =
                attr_static(&n.attrs, "for").or_else(|| attr_static(&n.attrs, "htmlfor"))
            && !target.trim().is_empty()
        {
            state.labeled_ids.insert(String::from(target.trim()));
        }
        collect_doc_landmarks_and_labels(&n.children, state);
    }
}

fn is_unlabeled_duplicate_landmark(node: &HtmlElementNode<'_>, state: &DocAuditState) -> bool {
    let multi = (node.tag == "nav" && state.total_nav_count > 1)
        || (node.tag == "aside" && state.total_aside_count > 1);
    multi && !has_attr(&node.attrs, "aria-label") && !has_attr(&node.attrs, "aria-labelledby")
}

fn check_landmark_and_id(
    node: &HtmlElementNode<'_>,
    state: &mut DocAuditState,
    out: &mut Vec<LangRuleFinding>,
) {
    if node.tag == "html" && attr_static(&node.attrs, "lang").is_none_or(|l| l.trim().is_empty()) {
        out.push(LangRuleFinding::new(
            W3cViolation::LandmarkAndIdUniqueness,
            node.span,
            String::from("`<html>` root element is missing a non-empty `lang` attribute"),
        ));
    }
    if node.tag == "main" && !has_attr(&node.attrs, "hidden") {
        state.visible_main_count += 1;
        if state.visible_main_count > 1 {
            out.push(LangRuleFinding::new(
                W3cViolation::LandmarkAndIdUniqueness,
                node.span,
                String::from("document contains multiple non-hidden `<main>` landmarks"),
            ));
        }
    }
    if is_unlabeled_duplicate_landmark(node, state) {
        out.push(LangRuleFinding::new(
            W3cViolation::LandmarkAndIdUniqueness,
            node.span,
            format!("multiple `<{}>` landmarks require distinct `aria-label` or `aria-labelledby` attributes", node.tag),
        ));
    }
    if let Some(trimmed) = attr_static(&node.attrs, "id")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && !state.seen_ids.insert(String::from(trimmed))
    {
        out.push(LangRuleFinding::new(
            W3cViolation::LandmarkAndIdUniqueness,
            node.span,
            format!("duplicate static `id=\"{trimmed}\"` attribute in document"),
        ));
    }
}

fn walk_element(
    node: &HtmlElementNode<'_>,
    anc: AncestorCtx<'_>,
    state: &mut DocAuditState,
    out: &mut Vec<LangRuleFinding>,
) {
    let intrinsic = is_intrinsic_tag(node.tag);
    if intrinsic {
        check_landmark_and_id(node, state, out);
        check_w3c_content_rules(node, anc, out);
        validate_element_aria(node.tag, &node.attrs, node.span, out);
        check_focus_and_tabindex(node, anc.has(ANC_ARIA_HIDDEN), out);
        check_media_and_form_control(node, anc.has(ANC_LABEL), &state.labeled_ids, out);
    }
    let self_hidden = attr_static(&node.attrs, "aria-hidden")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    let mut flags = anc.flags;
    for (cond, bit) in [
        (
            intrinsic && is_interactive_control(node.tag, &node.attrs),
            ANC_INTERACTIVE,
        ),
        (intrinsic && node.tag == "form", ANC_FORM),
        (intrinsic && matches!(node.tag, "p" | "span"), ANC_P_OR_SPAN),
        (intrinsic && node.tag == "label", ANC_LABEL),
        (self_hidden, ANC_ARIA_HIDDEN),
    ] {
        if cond {
            flags |= bit;
        }
    }
    let next_anc = AncestorCtx {
        parent_tag: intrinsic.then_some(node.tag),
        flags,
    };
    for child in &node.children {
        walk_element(child, next_anc, state, out);
    }
}

/// Evaluates all W3C HTML5 content-model and WCAG 2.2 AA rules on `roots`.
pub(in crate::langs) fn audit_html_element_tree(
    roots: &[HtmlElementNode<'_>],
    out: &mut Vec<LangRuleFinding>,
) {
    let mut state = DocAuditState {
        seen_ids: BTreeSet::new(),
        labeled_ids: BTreeSet::new(),
        visible_main_count: 0,
        total_nav_count: 0,
        total_aside_count: 0,
    };
    collect_doc_landmarks_and_labels(roots, &mut state);
    for root in roots {
        walk_element(root, AncestorCtx::default(), &mut state, out);
    }
}
