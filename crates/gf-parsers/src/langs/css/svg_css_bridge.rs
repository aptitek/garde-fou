//! Cross-AST bridge from SVG `<style>` elements and `style="..."` attributes into the W3C CSS
//! Syntax Level 3 analyzer (`GF000` / `GF064`).

use super::parser_ast::{CssRule, CssValue, parse_css_stylesheet, parse_inline_css_declarations};
use super::tokenizer::tokenize_css;
use super::value_rules::extract_url_payload;

pub struct SvgEmbeddedCssOutcome {
    pub syntax_errors: Vec<(u32, String)>,
    pub unsafe_hits: Vec<(u32, String)>,
}

const UNSAFE_SVG_URL_PREFIXES: &[&str] = &[
    "javascript:",
    "vbscript:",
    "data:",
    "http://",
    "https://",
    "//",
];

fn is_unsafe_svg_css_url(url: &str) -> bool {
    let low = url.to_ascii_lowercase();
    UNSAFE_SVG_URL_PREFIXES.iter().any(|p| low.starts_with(p))
}

/// Inspects embedded SVG `<style>` or `style="..."` CSS for syntax errors, `@import`, and unsafe `url(...)`.
pub fn inspect_svg_embedded_css(
    css_text: &str,
    inline_attr: bool,
    base_line: u32,
) -> SvgEmbeddedCssOutcome {
    let lexed = tokenize_css(css_text);
    let remap = |l: u32| base_line.saturating_add(l.saturating_sub(1));
    let syntax_errors: Vec<(u32, String)> = lexed
        .syntax_errors
        .into_iter()
        .map(|e| (remap(e.span.start_line()), e.message))
        .collect();
    let mut unsafe_hits = Vec::new();
    if inline_attr {
        for d in parse_inline_css_declarations(&lexed.tokens) {
            scan_svg_css_values(&d.values, remap, &mut unsafe_hits);
        }
    } else {
        for r in parse_css_stylesheet(&lexed.tokens) {
            scan_svg_css_rule(&r, remap, &mut unsafe_hits);
        }
    }
    SvgEmbeddedCssOutcome {
        syntax_errors,
        unsafe_hits,
    }
}

fn scan_svg_css_rule(
    rule: &CssRule,
    remap: impl Fn(u32) -> u32 + Copy,
    out: &mut Vec<(u32, String)>,
) {
    if let CssRule::AtRule {
        name,
        line,
        prelude,
        ..
    } = rule
    {
        if name == "import" {
            out.push((
                remap(*line),
                String::from("external `@import` inside SVG `<style>`"),
            ));
        }
        scan_svg_css_values(prelude, remap, out);
    }
    for d in rule.declarations() {
        scan_svg_css_values(&d.values, remap, out);
    }
    for nr in rule.nested_rules() {
        scan_svg_css_rule(nr, remap, out);
    }
}

fn scan_svg_css_values(
    values: &[CssValue],
    remap: impl Fn(u32) -> u32 + Copy,
    out: &mut Vec<(u32, String)>,
) {
    for val in values {
        let CssValue::Function { name, line, args } = val else {
            continue;
        };
        if name.eq_ignore_ascii_case("url")
            && let Some(u) = extract_url_payload(args)
            && is_unsafe_svg_css_url(u)
        {
            out.push((remap(*line), format!("unsafe `url({u})` inside SVG CSS")));
        }
        scan_svg_css_values(args, remap, out);
    }
}
