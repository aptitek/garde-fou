//! WCAG 2.2 AA focus, `tabIndex`, `aria-hidden`, form-control labeling, and media caption checks
//! (`A11Y-HIDDEN-FOCUSABLE`, `A11Y-TABINDEX-AND-FOCUS-RING`, `A11Y-MEDIA-FORM-MOTION` — `GF066`/`GF080`).

use std::collections::BTreeSet;

use gf_core::{A11yViolation, LangRuleFinding};

use super::aria_table::{HtmlAttrView, HtmlElementNode};

fn find_attr<'a>(attrs: &'a [HtmlAttrView<'a>], name: &str) -> Option<&'a HtmlAttrView<'a>> {
    attrs.iter().find(|a| a.name.eq_ignore_ascii_case(name))
}

pub(super) fn has_attr(attrs: &[HtmlAttrView<'_>], name: &str) -> bool {
    find_attr(attrs, name).is_some()
}

pub(super) fn attr_static<'a>(attrs: &'a [HtmlAttrView<'a>], name: &str) -> Option<&'a str> {
    find_attr(attrs, name).and_then(|a| a.static_val)
}

pub(super) fn is_hidden_input(tag: &str, attrs: &[HtmlAttrView<'_>]) -> bool {
    tag == "input"
        && attr_static(attrs, "type").is_some_and(|t| t.trim().eq_ignore_ascii_case("hidden"))
}

fn parse_tabindex(attrs: &[HtmlAttrView<'_>]) -> Option<i64> {
    attr_static(attrs, "tabindex")?.trim().parse::<i64>().ok()
}

fn is_inherently_focusable(tag: &str, attrs: &[HtmlAttrView<'_>]) -> bool {
    match tag {
        "button" | "select" | "textarea" => true,
        "a" => has_attr(attrs, "href"),
        "input" => !is_hidden_input(tag, attrs),
        _ => false,
    }
}

fn check_unpaired_hover(node: &HtmlElementNode<'_>, out: &mut Vec<LangRuleFinding>) {
    let unpaired_over = has_attr(&node.attrs, "onmouseover") && !has_attr(&node.attrs, "onfocus");
    let unpaired_out = has_attr(&node.attrs, "onmouseout") && !has_attr(&node.attrs, "onblur");
    if unpaired_over || unpaired_out {
        out.push(LangRuleFinding::new(
            A11yViolation::TabindexAndFocusRing,
            node.span,
            format!(
                "mouse hover handler on `<{}>` lacks matching `onFocus`/`onBlur` handler",
                node.tag
            ),
        ));
    }
}

pub(super) fn check_focus_and_tabindex(
    node: &HtmlElementNode<'_>,
    ancestor_aria_hidden: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let tab_idx = parse_tabindex(&node.attrs);
    if let Some(ti) = tab_idx.filter(|&v| v > 0) {
        out.push(LangRuleFinding::new(
            A11yViolation::TabindexAndFocusRing,
            node.span,
            format!(
                "positive `tabIndex={ti}` on `<{}>` disrupts natural focus order",
                node.tag
            ),
        ));
    }
    if has_attr(&node.attrs, "autofocus") {
        out.push(LangRuleFinding::new(
            A11yViolation::TabindexAndFocusRing,
            node.span,
            format!(
                "`autoFocus` on `<{}>` disorients screen-reader and keyboard users",
                node.tag
            ),
        ));
    }
    check_unpaired_hover(node, out);

    let self_aria_hidden = attr_static(&node.attrs, "aria-hidden")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    if (!self_aria_hidden && !ancestor_aria_hidden) || has_attr(&node.attrs, "disabled") {
        return;
    }
    let focusable_by_tag = is_inherently_focusable(node.tag, &node.attrs) && tab_idx != Some(-1);
    if focusable_by_tag || tab_idx.is_some_and(|ti| ti >= 0) {
        out.push(LangRuleFinding::new(
            A11yViolation::HiddenFocusable,
            node.span,
            format!(
                "`aria-hidden=\"true\"` on or around focusable `<{}>` traps keyboard focus",
                node.tag
            ),
        ));
    }
}

fn check_media_captions(node: &HtmlElementNode<'_>, out: &mut Vec<LangRuleFinding>) {
    if !matches!(node.tag, "video" | "audio") {
        return;
    }
    let is_muted = node.tag == "video" && has_attr(&node.attrs, "muted");
    let is_hidden = attr_static(&node.attrs, "aria-hidden")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    let has_captions = node.children.iter().any(|ch| {
        ch.tag == "track"
            && attr_static(&ch.attrs, "kind")
                .is_some_and(|k| matches!(k.trim(), "captions" | "subtitles"))
    });
    if !is_muted && !is_hidden && !has_captions {
        out.push(LangRuleFinding::new(
            A11yViolation::MediaFormMotion,
            node.span,
            format!(
                "`<{}>` media element is missing `<track kind=\"captions\">`",
                node.tag
            ),
        ));
    }
}

fn is_label_required_control(tag: &str, attrs: &[HtmlAttrView<'_>]) -> bool {
    match tag {
        "select" | "textarea" => true,
        "input" => !attr_static(attrs, "type").is_some_and(|t| {
            matches!(
                t.trim().to_ascii_lowercase().as_str(),
                "hidden" | "submit" | "reset" | "button" | "image"
            )
        }),
        _ => false,
    }
}

pub(super) fn check_media_and_form_control(
    node: &HtmlElementNode<'_>,
    in_label: bool,
    labeled_ids: &BTreeSet<String>,
    out: &mut Vec<LangRuleFinding>,
) {
    check_media_captions(node, out);
    let has_aria_label = ["aria-label", "aria-labelledby", "title"]
        .iter()
        .any(|k| has_attr(&node.attrs, k));
    if matches!(node.tag, "button" | "a") && !node.has_any_child_content && !has_aria_label {
        out.push(LangRuleFinding::new(
            A11yViolation::MediaFormMotion,
            node.span,
            format!(
                "empty interactive `<{}>` has no accessible text or `aria-label`",
                node.tag
            ),
        ));
        return;
    }
    if !is_label_required_control(node.tag, &node.attrs) || in_label {
        return;
    }
    let has_dynamic_id = node
        .attrs
        .iter()
        .any(|a| a.name.eq_ignore_ascii_case("id") && a.is_dynamic);
    let has_for_match = has_dynamic_id
        || attr_static(&node.attrs, "id").is_some_and(|id| labeled_ids.contains(id.trim()));
    if !has_aria_label && !has_for_match {
        out.push(LangRuleFinding::new(
            A11yViolation::MediaFormMotion,
            node.span,
            format!(
                "form control `<{}>` has no associated `<label>` or `aria-label`",
                node.tag
            ),
        ));
    }
}
