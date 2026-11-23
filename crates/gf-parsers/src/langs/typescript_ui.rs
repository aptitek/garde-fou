//! TypeScript / JavaScript style-leak (`GF060`), inline-asset (`GF064`), and `t(...)` (`GF062`)
//! AST extraction.

use gf_core::{
    DomainLayoutConfig, StyleLeakFinding, StyleLeakKind, UastDocument, classify_path,
    is_asset_extension,
};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const CSS_UNITS: &[&str] = &["px", "rem", "em", "vh", "vw", "vmin", "vmax"];

fn is_hex_color_token(tok: &str) -> bool {
    let Some(hex) = tok.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn contains_raw_color(val: &str) -> bool {
    let lower = val.to_ascii_lowercase();
    if ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "oklab("]
        .iter()
        .any(|fn_pfx| lower.contains(fn_pfx))
    {
        return true;
    }
    if lower.contains("http://") || lower.contains("https://") {
        return false;
    }
    val.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '(' | ')'))
        .any(is_hex_color_token)
}

fn is_standalone_css_unit(val: &str) -> bool {
    let trimmed = val.trim();
    for &unit in CSS_UNITS {
        if let Some(num) = trimmed.strip_suffix(unit)
            && !num.is_empty()
            && num
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            && num.chars().any(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

fn maybe_extract_asset_ref(unquoted: &str) -> Option<String> {
    let trimmed = unquoted.trim();
    if trimmed.is_empty()
        || trimmed.contains("://")
        || trimmed.starts_with("data:")
        || trimmed.contains(' ')
    {
        return None;
    }
    let clean = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let ext = clean.rsplit('.').next().unwrap_or("");
    if clean != ext && is_asset_extension(ext) {
        Some(String::from(clean))
    } else {
        None
    }
}

fn unquote_literal(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2
        && matches!(bytes[0], b'"' | b'\'' | b'`')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        raw.get(1..raw.len() - 1)
    } else {
        None
    }
}

fn is_translation_call(callee_text: &str) -> bool {
    let member = callee_text
        .rsplit_once('.')
        .map_or(callee_text, |(_, prop)| prop);
    member == "t" || member == "translate"
}

fn handle_call_expr(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let callee = node_text(func, source).trim();
    if callee.starts_with("styled.")
        || callee.starts_with("styled(")
        || callee == "css"
        || callee == "createGlobalStyle"
    {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::CssInJs,
            detail: format!("CSS-in-JS call `{callee}`"),
            span: node_span(node),
        });
        return;
    }
    if !is_translation_call(callee) {
        return;
    }
    let Some(args) = node.child_by_field_name("arguments") else {
        return;
    };
    let mut cursor = args.walk();
    if let Some(first_arg) = args.named_children(&mut cursor).next()
        && let Some(key) = unquote_literal(node_text(first_arg, source))
        && !key.is_empty()
    {
        doc.translation_keys_used.insert(String::from(key));
    }
}

fn handle_tagged_template(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(tag_node) = node.child_by_field_name("function") else {
        return;
    };
    let tag = node_text(tag_node, source).trim();
    if tag == "css"
        || tag == "createGlobalStyle"
        || tag.starts_with("styled.")
        || tag.starts_with("styled(")
    {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::CssInJs,
            detail: format!("CSS-in-JS tagged template `{tag}`"),
            span: node_span(node),
        });
    }
}

fn handle_string_node(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if node
        .parent()
        .is_some_and(|p| p.kind() == "import_statement")
    {
        if let Some(val) = unquote_literal(node_text(node, source))
            && let Some(asset) = maybe_extract_asset_ref(val)
        {
            doc.asset_refs.insert(asset);
        }
        return;
    }
    let Some(val) = unquote_literal(node_text(node, source)) else {
        return;
    };
    let span = node_span(node);
    if val.trim_start().starts_with("data:") {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::DataUri,
            detail: String::from("inline `data:` URI in code"),
            span,
        });
    } else if let Some(asset) = maybe_extract_asset_ref(val) {
        doc.asset_refs.insert(asset);
    } else if contains_raw_color(val) {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::RawColorLiteral,
            detail: format!("raw color literal `{val}`"),
            span,
        });
    } else if is_standalone_css_unit(val) {
        doc.style_leaks.push(StyleLeakFinding {
            kind: StyleLeakKind::RawCssUnit,
            detail: format!("hardcoded CSS unit `{val}`"),
            span,
        });
    }
}

/// Extracts CSS-in-JS leaks, raw colors/units, `t("...")` keys, and asset refs from TS/JS ASTs.
pub fn extract_ts_style_and_i18n(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let _ = classify_path(&doc.path, &DomainLayoutConfig::strict_defaults());
    walk_style_i18n(root, source, doc);
}

fn walk_style_i18n(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "call_expression" => handle_call_expr(node, source, doc),
        "tagged_template_expression" => handle_tagged_template(node, source, doc),
        "assignment_expression" => {
            if let Some(lhs) = node.child_by_field_name("left") {
                let text = node_text(lhs, source);
                let tail_style = text.rsplit_once('.').is_some_and(|(_, p)| p == "style");
                if text.contains(".style.") || tail_style {
                    doc.style_leaks.push(StyleLeakFinding {
                        kind: StyleLeakKind::CssInJs,
                        detail: format!("imperative DOM style mutation `{text}`"),
                        span: node_span(node),
                    });
                }
            }
        }
        "string" | "template_string" => handle_string_node(node, source, doc),
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_style_i18n(child, source, doc);
    }
}
