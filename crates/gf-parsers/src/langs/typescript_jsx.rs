//! JSX / TSX extraction for inline styles (`GF060`), inline `<svg>` (`GF064`),
//! untranslated UI text (`GF062`), and static accessibility (`GF066`).

use gf_core::{StyleLeakFinding, StyleLeakKind, UastDocument, UiCheckFinding, UiCheckKind};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const TRANSLATABLE_ATTRS: &[&str] = &["placeholder", "title", "alt", "aria-label"];
const NON_INTERACTIVE_TAGS: &[&str] = &["div", "span", "p"];

fn has_two_letters(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).take(2).count() >= 2
}

fn jsx_attr_name_and_val<'a>(
    attr: Node<'a>,
    source: &'a str,
) -> Option<(&'a str, Option<Node<'a>>)> {
    if attr.kind() != "jsx_attribute" {
        return None;
    }
    let mut cursor = attr.walk();
    let mut children = attr.named_children(&mut cursor);
    let name_node = children.next()?;
    let val_node = children.next();
    Some((node_text(name_node, source).trim(), val_node))
}

fn collect_element_attrs<'a>(
    tag_node: Node<'a>,
    source: &'a str,
) -> Vec<(&'a str, Option<Node<'a>>)> {
    let mut out = Vec::new();
    let mut cursor = tag_node.walk();
    for child in tag_node.named_children(&mut cursor) {
        if let Some(pair) = jsx_attr_name_and_val(child, source) {
            out.push(pair);
        }
    }
    out
}

fn check_jsx_attributes(
    attrs: &[(&str, Option<Node<'_>>)],
    span: gf_core::Span,
    source: &str,
    doc: &mut UastDocument,
) {
    for &(name, val_opt) in attrs {
        if name == "style" {
            doc.style_leaks.push(StyleLeakFinding {
                kind: StyleLeakKind::InlineStyleAttr,
                detail: String::from("inline JSX `style` attribute"),
                span,
            });
        } else if TRANSLATABLE_ATTRS.contains(&name)
            && let Some(val_node) = val_opt
            && val_node.kind() == "string"
        {
            let raw = node_text(val_node, source).trim_matches(['"', '\'']);
            if has_two_letters(raw) {
                doc.ui_checks.push(UiCheckFinding {
                    kind: UiCheckKind::UntranslatedText,
                    detail: format!("hardcoded JSX attribute `{name}=\"{raw}\"` outside `t(...)`"),
                    span: node_span(val_node),
                });
            }
        }
    }
}

fn has_accessible_button_children(elem_node: Node<'_>, source: &str) -> bool {
    if elem_node.kind() == "jsx_self_closing_element" {
        return false;
    }
    let mut cursor = elem_node.walk();
    for child in elem_node.named_children(&mut cursor) {
        match child.kind() {
            "jsx_text" if !node_text(child, source).trim().is_empty() => return true,
            "jsx_expression" => return true,
            _ => {}
        }
    }
    false
}

fn check_jsx_button(
    elem_node: Node<'_>,
    attrs: &[(&str, Option<Node<'_>>)],
    source: &str,
    doc: &mut UastDocument,
) {
    let span = node_span(elem_node);
    let has_attr = |target: &str| attrs.iter().any(|(n, _)| *n == target);
    if !has_attr("type") {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ButtonMissingType,
            detail: String::from(
                "`<button>` missing explicit `type` (`button` | `submit` | `reset`)",
            ),
            span,
        });
    }
    let labelled = has_attr("aria-label")
        || has_attr("aria-labelledby")
        || has_accessible_button_children(elem_node, source);
    if !labelled {
        doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ButtonMissingLabel,
            detail: String::from("`<button>` has no accessible label or text content"),
            span,
        });
    }
}

fn is_inaccessible_click_element(tag: &str, attrs: &[(&str, Option<Node<'_>>)]) -> bool {
    let has_attr = |cand: &str| attrs.iter().any(|(n, _)| *n == cand);
    NON_INTERACTIVE_TAGS.contains(&tag)
        && has_attr("onClick")
        && !(has_attr("role") && (has_attr("tabIndex") || has_attr("onKeyDown")))
}

fn check_jsx_tag_semantics(
    target: (&str, Node<'_>),
    attrs: &[(&str, Option<Node<'_>>)],
    source: &str,
    doc: &mut UastDocument,
) {
    let (tag, elem_node) = target;
    let span = node_span(elem_node);
    let has_alt = attrs.iter().any(|(n, _)| *n == "alt");

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
        "img" if !has_alt => doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::ImgMissingAlt,
            detail: String::from("`<img>` element is missing required `alt` attribute"),
            span,
        }),
        "button" => check_jsx_button(elem_node, attrs, source, doc),
        _ if is_inaccessible_click_element(tag, attrs) => doc.ui_checks.push(UiCheckFinding {
            kind: UiCheckKind::NonInteractiveClick,
            detail: format!("non-interactive `<{tag} onClick>` without `role` + keyboard support"),
            span,
        }),
        _ => {}
    }
}

fn inspect_jsx_element(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let header = if node.kind() == "jsx_element" {
        node.child_by_field_name("open_tag")
    } else {
        Some(node)
    };
    let Some(tag_holder) = header else {
        return;
    };
    let Some(name_node) = tag_holder.child_by_field_name("name") else {
        return;
    };
    let tag_name = node_text(name_node, source).trim();
    let attrs = collect_element_attrs(tag_holder, source);
    check_jsx_attributes(&attrs, node_span(tag_holder), source, doc);
    check_jsx_tag_semantics((tag_name, node), &attrs, source, doc);
}

/// Walks JSX/TSX CST nodes to populate `style_leaks` and `ui_checks`.
pub fn extract_jsx_ui_and_a11y(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_jsx(root, source, doc);
}

fn walk_jsx(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "jsx_element" | "jsx_self_closing_element" => inspect_jsx_element(node, source, doc),
        "jsx_text" => {
            let trimmed = node_text(node, source).trim();
            if has_two_letters(trimmed) {
                doc.ui_checks.push(UiCheckFinding {
                    kind: UiCheckKind::UntranslatedText,
                    detail: format!("hardcoded JSX text `{trimmed}` outside `t(...)`"),
                    span: node_span(node),
                });
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_jsx(child, source, doc);
    }
}
