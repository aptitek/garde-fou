//! Declaration, selector prelude, `var()` fallback, `calc()` dimension, and URL analyzers (`GF061`).

use std::collections::BTreeSet;

use gf_core::{CssViolation, FormeFinding, FormeFindingKind, Span};

use super::colors::{is_css_named_color, is_direct_color_function, is_valid_hex_color};
use super::parser_ast::{CssDeclaration, CssValue};
use super::tokenizer::{CssTokKind, CssToken};

const FORBIDDEN_LENGTH_UNITS: &[&str] = &["px", "pt", "cm", "mm", "in"];

pub(super) use super::css_ast_nodes::extract_url_payload;

fn line_span(line: u32) -> Span {
    Span::new(line, line).unwrap_or(Span::FIRST)
}

pub(super) fn push_css_spec(
    findings: &mut Vec<FormeFinding>,
    violation: CssViolation,
    line: u32,
    detail: String,
) {
    findings.push(FormeFinding {
        kind: FormeFindingKind::CssSpec(violation),
        detail,
        span: line_span(line),
    });
}

pub(super) fn record_url_target(
    url_str: &str,
    line: u32,
    findings: &mut Vec<FormeFinding>,
    asset_refs: &mut BTreeSet<String>,
) {
    let lower = url_str.to_ascii_lowercase();
    if lower.starts_with("data:") {
        findings.push(FormeFinding {
            kind: FormeFindingKind::DataUri,
            detail: String::from("embedded `url(data:...)` in stylesheet"),
            span: line_span(line),
        });
    } else if !url_str.is_empty()
        && !url_str.contains("://")
        && !url_str.starts_with("//")
        && !lower.starts_with("javascript:")
        && !url_str.starts_with('#')
    {
        let clean = url_str.split(['?', '#']).next().unwrap_or(url_str);
        if !clean.is_empty() {
            asset_refs.insert(String::from(clean));
        }
    }
}

fn check_content_property(decl: &CssDeclaration, findings: &mut Vec<FormeFinding>) {
    for val in &decl.values {
        if let CssValue::Token(CssToken {
            kind: CssTokKind::Str(text),
            line,
        }) = val
            && text.chars().filter(|c| c.is_alphabetic()).count() >= 2
        {
            findings.push(FormeFinding {
                kind: FormeFindingKind::TranslatedContentText,
                detail: format!("translatable text in CSS `content: \"{text}\"`"),
                span: line_span(*line),
            });
        }
    }
}

fn check_transition_all(decl: &CssDeclaration, findings: &mut Vec<FormeFinding>) {
    for val in &decl.values {
        if let CssValue::Token(CssToken {
            kind: CssTokKind::Ident(id),
            line,
        }) = val
            && id.eq_ignore_ascii_case("all")
        {
            push_css_spec(
                findings,
                CssViolation::NoTransitionAll,
                *line,
                format!("`{}: all` animates layout/paint properties", decl.property),
            );
        }
    }
}

fn check_z_index(decl: &CssDeclaration, findings: &mut Vec<FormeFinding>) {
    for val in &decl.values {
        if let CssValue::Token(CssToken {
            kind: CssTokKind::Number(n),
            line,
        }) = val
            && !matches!(n.as_str(), "-1" | "0" | "1")
        {
            push_css_spec(
                findings,
                CssViolation::RawDimensionOrZIndex,
                *line,
                format!("raw `z-index: {n}` outside `var(--z-*)` elevation scale"),
            );
        }
    }
}

struct ValueScanCtx<'a> {
    skip_named_colors: bool,
    in_var_fallback: bool,
    findings: &'a mut Vec<FormeFinding>,
    asset_refs: &'a mut BTreeSet<String>,
}

impl ValueScanCtx<'_> {
    fn emit_prim(&mut self, base: CssViolation, line: u32, detail: String) {
        let v = if self.in_var_fallback {
            CssViolation::FallbackTokenLeak
        } else {
            base
        };
        push_css_spec(self.findings, v, line, detail);
    }

    fn emit_color(&mut self, line: u32, label: &str) {
        self.emit_prim(
            CssViolation::RawColor,
            line,
            format!("raw color `{label}` outside Design Tokens"),
        );
    }

    fn emit_dim(&mut self, line: u32, num: &str, unit: &str) {
        self.emit_prim(
            CssViolation::RawDimensionOrZIndex,
            line,
            format!("raw dimension `{num}{unit}` outside Design Tokens"),
        );
    }

    fn scan_var_args(&mut self, args: &[CssValue]) {
        let mut after_comma = false;
        for arg in args {
            if matches!(
                arg,
                CssValue::Token(CssToken {
                    kind: CssTokKind::Comma,
                    ..
                })
            ) {
                after_comma = true;
            } else if after_comma {
                let prev = self.in_var_fallback;
                self.in_var_fallback = true;
                self.scan_value(arg);
                self.in_var_fallback = prev;
            }
        }
    }

    fn scan_token_value(&mut self, tok: &CssToken) {
        match &tok.kind {
            CssTokKind::Hash(h) if is_valid_hex_color(h) => {
                self.emit_color(tok.line, &format!("#{h}"));
            }
            CssTokKind::Ident(id)
                if !self.skip_named_colors && !id.starts_with("--") && is_css_named_color(id) =>
            {
                self.emit_color(tok.line, id);
            }
            CssTokKind::Dimension { num, unit }
                if FORBIDDEN_LENGTH_UNITS.contains(&unit.as_str()) =>
            {
                self.emit_dim(tok.line, num, unit);
            }
            _ => {}
        }
    }

    fn scan_function_value(&mut self, name: &str, line: u32, args: &[CssValue]) {
        let lower = name.to_ascii_lowercase();
        if lower == "url" {
            if let Some(u) = extract_url_payload(args) {
                record_url_target(u, line, self.findings, self.asset_refs);
            }
        } else if is_direct_color_function(&lower) {
            self.emit_color(line, &format!("{name}(...)"));
        } else if lower == "var" {
            self.scan_var_args(args);
        } else {
            for arg in args {
                self.scan_value(arg);
            }
        }
    }

    fn scan_value(&mut self, val: &CssValue) {
        match val {
            CssValue::Token(tok) => self.scan_token_value(tok),
            CssValue::Function { name, line, args } => self.scan_function_value(name, *line, args),
        }
    }
}

pub(super) fn analyze_declaration(
    decl: &CssDeclaration,
    findings: &mut Vec<FormeFinding>,
    asset_refs: &mut BTreeSet<String>,
) {
    let prop_lower = decl.property.to_ascii_lowercase();
    if decl.important {
        push_css_spec(
            findings,
            CssViolation::NoImportant,
            decl.line,
            format!("`!important` override on `{}`", decl.property),
        );
    }
    if prop_lower == "content" {
        check_content_property(decl, findings);
    }
    if matches!(prop_lower.as_str(), "transition" | "transition-property") {
        check_transition_all(decl, findings);
    }
    if prop_lower == "z-index" {
        check_z_index(decl, findings);
    }
    let skip_named_colors = matches!(
        prop_lower.as_str(),
        "font-family" | "animation-name" | "grid-area" | "content"
    );
    let mut ctx = ValueScanCtx {
        skip_named_colors,
        in_var_fallback: false,
        findings,
        asset_refs,
    };
    for v in &decl.values {
        ctx.scan_value(v);
    }
}
