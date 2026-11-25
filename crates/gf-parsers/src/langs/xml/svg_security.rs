//! Post-entity-decoding SVG DOM security analyzer (`SVG-DANGEROUS-ELEMENT`,
//! `SVG-EVENT-OR-SMIL-XSS`, `SVG-UNSAFE-HREF-OR-USE`) and `<style>` / `style=` CSS AST bridge.

use gf_core::{AssetHazard, AssetHazardKind, Span, SyntaxErrorNode, XmlSvgViolation};
use roxmltree::{Attribute, Document, Node};

use crate::langs::css::{SvgEmbeddedCssOutcome, inspect_svg_embedded_css};

const DANGEROUS_SVG_ELEMENTS: &[&str] = &[
    "script",
    "foreignobject",
    "iframe",
    "embed",
    "object",
    "handler",
];

const SMIL_ANIMATION_ELEMENTS: &[&str] = &["set", "animate", "animatetransform", "animatemotion"];

fn span_at(doc: &Document<'_>, byte_offset: usize) -> Span {
    let row = doc.text_pos_at(byte_offset).row.max(1);
    Span::new(row, row).unwrap_or(Span::FIRST)
}

fn push_svg_hazard(
    hazards: &mut Vec<AssetHazard>,
    violation: XmlSvgViolation,
    span: Span,
    detail: String,
) {
    hazards.push(AssetHazard {
        kind: AssetHazardKind::XmlSvgSpec(violation),
        detail,
        span,
    });
}

fn clean_uri_scheme_probe(val: &str) -> String {
    val.chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_ascii_control())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_dangerous_uri_scheme(val: &str) -> bool {
    let probe = clean_uri_scheme_probe(val);
    probe.starts_with("javascript:") || probe.starts_with("vbscript:") || probe.starts_with("data:")
}

fn is_external_http_uri(val: &str) -> bool {
    let trimmed = val.trim().to_ascii_lowercase();
    trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("//")
}

fn is_href_attr(attr: &Attribute<'_, '_>) -> bool {
    let lower = attr.name().to_ascii_lowercase();
    lower == "href" || lower == "xlink:href"
}

fn check_smil_element(doc: &Document<'_>, node: Node<'_, '_>, hazards: &mut Vec<AssetHazard>) {
    let el_span = span_at(doc, node.range().start);
    for attr in node.attributes() {
        let name_low = attr.name().to_ascii_lowercase();
        let val_trim = attr.value().trim().to_ascii_lowercase();
        if name_low == "attributename"
            && (val_trim.starts_with("on") || val_trim == "href" || val_trim == "xlink:href")
        {
            push_svg_hazard(
                hazards,
                XmlSvgViolation::SvgEventOrSmilXss,
                span_at(doc, attr.range().start),
                format!(
                    "SMIL `<{}>` mutates sensitive attribute `{}`",
                    node.tag_name().name(),
                    attr.value().trim()
                ),
            );
        } else if matches!(name_low.as_str(), "to" | "from" | "values")
            && is_dangerous_uri_scheme(attr.value())
        {
            push_svg_hazard(
                hazards,
                XmlSvgViolation::SvgEventOrSmilXss,
                el_span,
                format!(
                    "SMIL `<{}>` injects executable URI payload",
                    node.tag_name().name()
                ),
            );
        }
    }
}

struct SvgAuditOut<'a> {
    hazards: &'a mut Vec<AssetHazard>,
    syntax_errors: &'a mut Vec<SyntaxErrorNode>,
}

fn check_href_attribute(
    attr: &roxmltree::Attribute<'_, '_>,
    tag_lower: &str,
    attr_span: Span,
    hazards: &mut Vec<AssetHazard>,
) {
    if is_dangerous_uri_scheme(attr.value()) {
        push_svg_hazard(
            hazards,
            XmlSvgViolation::SvgUnsafeHrefOrUse,
            attr_span,
            format!("unsafe `{}` URI in SVG", attr.name()),
        );
    } else if tag_lower == "use" && is_external_http_uri(attr.value()) {
        push_svg_hazard(
            hazards,
            XmlSvgViolation::SvgUnsafeHrefOrUse,
            attr_span,
            format!(
                "external `<use {}=\"{}\">` reference",
                attr.name(),
                attr.value().trim()
            ),
        );
    }
}

fn check_element_attributes(
    doc: &Document<'_>,
    node: Node<'_, '_>,
    tag_lower: &str,
    out: &mut SvgAuditOut<'_>,
) {
    for attr in node.attributes() {
        let attr_span = span_at(doc, attr.range().start);
        let attr_lower = attr.name().to_ascii_lowercase();
        if attr_lower.len() > 2 && attr_lower.starts_with("on") {
            push_svg_hazard(
                out.hazards,
                XmlSvgViolation::SvgEventOrSmilXss,
                attr_span,
                format!("inline SVG event attribute `{}=`", attr.name()),
            );
        } else if is_href_attr(&attr) {
            check_href_attribute(&attr, tag_lower, attr_span, out.hazards);
        } else if attr_lower == "style" {
            bridge_svg_css(attr.value(), true, attr_span.start_line(), out);
        }
    }
}

fn bridge_svg_css(css_text: &str, inline_attr: bool, base_line: u32, out: &mut SvgAuditOut<'_>) {
    let bridged: SvgEmbeddedCssOutcome = inspect_svg_embedded_css(css_text, inline_attr, base_line);
    for (line, msg) in bridged.syntax_errors {
        out.syntax_errors.push(SyntaxErrorNode {
            span: Span::new(line, line).unwrap_or(Span::FIRST),
            message: msg,
        });
    }
    for (line, detail) in bridged.unsafe_hits {
        push_svg_hazard(
            out.hazards,
            XmlSvgViolation::SvgUnsafeHrefOrUse,
            Span::new(line, line).unwrap_or(Span::FIRST),
            detail,
        );
    }
}

fn collect_style_element_body(
    doc: &Document<'_>,
    node: Node<'_, '_>,
    fallback_line: u32,
) -> (String, u32) {
    let first_line = node
        .children()
        .find(Node::is_text)
        .map_or(fallback_line, |c| {
            span_at(doc, c.range().start).start_line()
        });
    let mut body = String::new();
    for child in node.children().filter(Node::is_text) {
        if let Some(t) = child.text() {
            body.push_str(t);
        }
    }
    (body, first_line)
}

/// Evaluates post-entity-decoded SVG security rules and `<style>` CSS bridges on `dom`.
pub fn analyze_svg_dom(
    dom: &Document<'_>,
    hazards: &mut Vec<AssetHazard>,
    syntax_errors: &mut Vec<SyntaxErrorNode>,
) {
    let mut out = SvgAuditOut {
        hazards,
        syntax_errors,
    };
    for node in dom.descendants().filter(Node::is_element) {
        let tag_lower = node.tag_name().name().to_ascii_lowercase();
        let el_span = span_at(dom, node.range().start);
        if DANGEROUS_SVG_ELEMENTS.contains(&tag_lower.as_str()) {
            push_svg_hazard(
                out.hazards,
                XmlSvgViolation::SvgDangerousElement,
                el_span,
                format!("dangerous `<{}>` element in SVG", node.tag_name().name()),
            );
        }
        if SMIL_ANIMATION_ELEMENTS.contains(&tag_lower.as_str()) {
            check_smil_element(dom, node, out.hazards);
        }
        check_element_attributes(dom, node, &tag_lower, &mut out);
        if tag_lower == "style" {
            let (body, first_line) = collect_style_element_body(dom, node, el_span.start_line());
            if !body.is_empty() {
                bridge_svg_css(&body, false, first_line, &mut out);
            }
        }
    }
}
