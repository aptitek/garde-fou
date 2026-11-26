//! Specification-grade W3C CSS & Design Token extractor (`GF061`, `GF000`, `GF070`).

pub mod colors;
mod css_ast_nodes;
mod css_tokens;
pub mod parser_ast;
mod svg_css_bridge;
pub mod tokenizer;
mod value_rules;

use std::collections::BTreeSet;

use gf_core::{
    A11yViolation, CssViolation, FormeDocument, FormeFinding, LangRuleFinding, Language, RepoPath,
    Span, UastDocument, WebEdgeSummary,
};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};
use parser_ast::{CssRule, CssValue, parse_css_stylesheet};
pub(crate) use svg_css_bridge::{SvgEmbeddedCssOutcome, inspect_svg_embedded_css};
use tokenizer::{CssTokKind, CssToken, tokenize_css};
use value_rules::{analyze_declaration, extract_url_payload, push_css_spec, record_url_target};

fn check_selector_prelude(prelude: &[CssValue], findings: &mut Vec<FormeFinding>) {
    for val in prelude {
        match val {
            CssValue::Token(CssToken {
                kind: CssTokKind::Hash(id),
                line,
            }) => push_css_spec(
                findings,
                CssViolation::NoIdSelector,
                *line,
                format!("ID selector `#{id}` elevates specificity"),
            ),
            CssValue::Function { args, .. } => check_selector_prelude(args, findings),
            CssValue::Token(_) => {}
        }
    }
}

/// Specification-grade stylesheet extractor for `.css` and `.scss` files (`DomainKind::Forme`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CssExtractor;

impl LanguageExtractor for CssExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Css, total_lines, effective_lines)?;

        let lexed = tokenize_css(source);
        doc.syntax_errors = lexed.syntax_errors;
        doc.bypass_findings = lexed.bypasses;

        let rules = parse_css_stylesheet(&lexed.tokens);
        let mut findings = Vec::new();
        for rule in &rules {
            analyze_css_rule(rule, &mut findings, &mut doc.asset_refs);
        }
        check_css_a11y_focus_and_motion(source, &rules, &mut doc);
        doc.forme = Some(FormeDocument { findings });
        Ok(doc)
    }
}

fn is_outline_none_or_zero(values: &[CssValue]) -> bool {
    values.iter().any(|v| match v {
        CssValue::Token(CssToken {
            kind: CssTokKind::Ident(id),
            ..
        }) => id.eq_ignore_ascii_case("none"),
        CssValue::Token(CssToken {
            kind: CssTokKind::Number(num) | CssTokKind::Dimension { num, .. },
            ..
        }) => num == "0",
        _ => false,
    })
}

fn collect_css_a11y_rules(
    rule: &CssRule,
    has_focus_visible: bool,
    motion_spans: &mut Vec<Span>,
    out: &mut Vec<LangRuleFinding>,
) {
    if let CssRule::AtRule { name, line, .. } = rule
        && name.eq_ignore_ascii_case("keyframes")
    {
        motion_spans.push(Span::new(*line, *line).unwrap_or(Span::FIRST));
    }
    for d in rule.declarations() {
        let span = Span::new(d.line, d.line).unwrap_or(Span::FIRST);
        if d.property == "outline" && is_outline_none_or_zero(&d.values) && !has_focus_visible {
            out.push(LangRuleFinding::new(
                A11yViolation::TabindexAndFocusRing,
                span,
                String::from(
                    "`outline: none`/`0` removes focus ring without a `:focus-visible` style",
                ),
            ));
        }
        if matches!(d.property.as_str(), "animation" | "animation-name")
            && !is_outline_none_or_zero(&d.values)
        {
            motion_spans.push(span);
        }
    }
    for nr in rule.nested_rules() {
        collect_css_a11y_rules(nr, has_focus_visible, motion_spans, out);
    }
}

fn check_css_a11y_focus_and_motion(source: &str, rules: &[CssRule], doc: &mut UastDocument) {
    let low = source.to_ascii_lowercase();
    let has_focus_visible = low.contains(":focus-visible");
    let has_reduced_motion = low.contains("prefers-reduced-motion");
    let mut motion_spans = Vec::new();
    for rule in rules {
        collect_css_a11y_rules(
            rule,
            has_focus_visible,
            &mut motion_spans,
            &mut doc.lang_rule_findings,
        );
    }
    if !has_reduced_motion {
        for &span in &motion_spans {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                A11yViolation::MediaFormMotion,
                span,
                String::from(
                    "CSS `@keyframes`/animation lacks `@media (prefers-reduced-motion: reduce)` guard",
                ),
            ));
        }
    }
    let we = doc.web_edge.get_or_insert_with(WebEdgeSummary::default);
    we.forme_has_reduced_motion = has_reduced_motion;
    we.forme_spatial_motion_spans = motion_spans;
}

fn analyze_at_import(
    line: u32,
    prelude: &[CssValue],
    findings: &mut Vec<FormeFinding>,
    asset_refs: &mut BTreeSet<String>,
) {
    push_css_spec(
        findings,
        CssViolation::NoAtImport,
        line,
        String::from("runtime `@import` stylesheet waterfall"),
    );
    for v in prelude {
        if let CssValue::Function {
            name: fn_n, args, ..
        } = v
            && fn_n.eq_ignore_ascii_case("url")
            && let Some(u) = extract_url_payload(args)
        {
            record_url_target(u, line, findings, asset_refs);
        } else if let CssValue::Token(CssToken {
            kind: CssTokKind::Str(s),
            ..
        }) = v
        {
            record_url_target(s, line, findings, asset_refs);
        }
    }
}

fn analyze_css_rule(
    rule: &CssRule,
    findings: &mut Vec<FormeFinding>,
    asset_refs: &mut BTreeSet<String>,
) {
    match rule {
        CssRule::AtRule {
            name,
            line,
            prelude,
            ..
        } => {
            if name == "import" {
                analyze_at_import(*line, prelude, findings, asset_refs);
            }
        }
        CssRule::QualifiedRule { prelude, .. } => check_selector_prelude(prelude, findings),
    }
    for d in rule.declarations() {
        analyze_declaration(d, findings, asset_refs);
    }
    for nr in rule.nested_rules() {
        analyze_css_rule(nr, findings, asset_refs);
    }
}
