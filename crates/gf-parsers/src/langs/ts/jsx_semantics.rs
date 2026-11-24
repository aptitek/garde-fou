//! JSX / TSX intrinsic lowercase DOM vs custom `PascalCase` component semantics:
//! `a11y` (`GF066`), `i18n` (`GF062`), and `<style>` / `<svg>` (`GF060` / `GF064`).

use gf_core::{StyleLeakFinding, StyleLeakKind, UastDocument, UiCheckFinding, UiCheckKind};
use tree_sitter::Node;

use super::jsx_dangerous::{
    JsxAttr, JsxElementRef, check_dangerous_jsx_and_style_attrs, collect_element_attrs,
    has_two_letters, static_attr_string,
};
use crate::cst::{node_span, node_text};

const NON_INTERACTIVE_TAGS: &[&str] = &["div", "span", "p"];
const CODE_TEXT_TAGS: &[&str] = &["code", "pre", "kbd", "samp", "style", "script"];

fn is_decorative_img(attrs: &[JsxAttr<'_>], source: &str) -> bool {
    let aria_hidden = attrs.iter().any(|a| {
        a.name == "aria-hidden"
            && (static_attr_string(a.value, source) == Some("true")
                || a.value
                    .is_some_and(|v| node_text(v, source).replace(' ', "") == "{true}"))
    });
    let presentation_role = attrs.iter().any(|a| {
        a.name == "role"
            && static_attr_string(a.value, source)
                .is_some_and(|r| matches!(r, "presentation" | "none"))
    });
    aria_hidden || presentation_role
}

fn check_intrinsic_img(
    elem_node: Node<'_>,
    attrs: &[JsxAttr<'_>],
    source: &str,
    doc: &mut UastDocument,
) {
    let span = node_span(elem_node);
    let Some(alt_attr) = attrs.iter().find(|a| a.name == "alt") else {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ImgMissingAlt,
            detail: String::from("`<img>` element is missing required `alt` attribute"),
            span,
        });
        return;
    };
    let empty_alt = alt_attr.value.is_none()
        || static_attr_string(alt_attr.value, source).is_some_and(|s| s.trim().is_empty());
    if empty_alt && !is_decorative_img(attrs, source) {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ImgMissingAlt,
            detail: String::from(
                "`<img>` has empty `alt=\"\"` without `aria-hidden=\"true\"` or `role=\"presentation\"`",
            ),
            span,
        });
    }
}

fn check_intrinsic_button(
    elem: JsxElementRef<'_>,
    attrs: &[JsxAttr<'_>],
    source: &str,
    doc: &mut UastDocument,
) {
    let span = node_span(elem.elem_node);
    let has_attr = |target: &str| attrs.iter().any(|a| a.name == target);
    let has_spread = {
        let mut c = elem.tag_holder.walk();
        elem.tag_holder
            .named_children(&mut c)
            .any(|n| n.kind() == "jsx_spread_element")
    };
    if !has_attr("type") && !has_spread {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ButtonMissingType,
            detail: String::from(
                "`<button>` missing explicit `type` (`button` | `submit` | `reset`)",
            ),
            span,
        });
    }
    let has_child = {
        let mut c = elem.elem_node.walk();
        elem.elem_node.named_children(&mut c).any(|ch| {
            matches!(
                ch.kind(),
                "jsx_expression" | "jsx_element" | "jsx_self_closing_element"
            ) || (ch.kind() == "jsx_text" && !node_text(ch, source).trim().is_empty())
        })
    };
    if !has_attr("aria-label") && !has_attr("aria-labelledby") && !has_child {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ButtonMissingLabel,
            detail: String::from("`<button>` has no accessible label or text content"),
            span,
        });
    }
}

fn inspect_jsx_element(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let header = if node.kind() == "jsx_element" {
        node.child_by_field_name("open_tag")
    } else {
        Some(node)
    };
    let Some(tag_holder) = header else { return };
    let Some(name_node) = tag_holder.child_by_field_name("name") else {
        return;
    };
    let tag = node_text(name_node, source).trim();
    let elem = JsxElementRef {
        tag,
        elem_node: node,
        tag_holder,
    };
    let attrs = collect_element_attrs(tag_holder, source);
    check_dangerous_jsx_and_style_attrs(elem, &attrs, source, doc);
    let span = node_span(node);
    match tag {
        "style" => doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::StyleElement,
            detail: String::from("embedded `<style>` tag inside JSX"),
            span,
        }),
        "svg" => doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::InlineSvg,
            detail: String::from("inline `<svg>` in JSX (extract to `assets/*.svg`)"),
            span,
        }),
        "img" => check_intrinsic_img(node, &attrs, source, doc),
        "button" => check_intrinsic_button(elem, &attrs, source, doc),
        t if NON_INTERACTIVE_TAGS.contains(&t)
            && attrs.iter().any(|a| a.name == "onClick")
            && !(attrs.iter().any(|a| a.name == "role")
                && attrs
                    .iter()
                    .any(|a| matches!(a.name, "tabIndex" | "onKeyDown" | "onKeyUp"))) =>
        {
            doc.ui_checks.push(UiCheckFinding {
                kind: UiCheckKind::NonInteractiveClick,
                detail: format!(
                    "non-interactive `<{t} onClick>` without `role` + keyboard support"
                ),
                span,
            });
        }
        _ => {}
    }
}

/// Walks JSX/TSX CST nodes for intrinsic `a11y` (`GF066`), `i18n` (`GF062`), and `REACT-NO-DANGEROUS-JSX`.
pub(super) fn extract_jsx_semantics(
    root: Node<'_>,
    source: &str,
    in_code_tag: bool,
    doc: &mut UastDocument,
) {
    let mut next_in_code = in_code_tag;
    match root.kind() {
        "jsx_element" | "jsx_self_closing_element" => {
            inspect_jsx_element(root, source, doc);
            if let Some(open) = root.child_by_field_name("open_tag")
                && let Some(nm) = open.child_by_field_name("name")
                && CODE_TEXT_TAGS.contains(&node_text(nm, source).trim())
            {
                next_in_code = true;
            }
        }
        "jsx_text" if !in_code_tag => {
            let trimmed = node_text(root, source).trim();
            if has_two_letters(trimmed) {
                doc.ui_checks.push(UiCheckFinding {
                    kind: UiCheckKind::UntranslatedText,
                    detail: format!("hardcoded JSX text `{trimmed}` outside `t(...)`"),
                    span: node_span(root),
                });
            }
        }
        _ => {}
    }
    let mut cursor = root.walk();
    for ch in root.children(&mut cursor) {
        extract_jsx_semantics(ch, source, next_in_code, doc);
    }
}
