//! Lexical CSS token types, balanced delimiter checker (`GF000`), and comment anti-bypass (`GF070`).

use std::str::Chars;

use gf_core::{BypassFinding, Span, SyntaxErrorNode};

/// Lexical token kinds produced by the W3C CSS Syntax Level 3 lexer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssTokKind {
    /// `@<ident>` at-rule keyword (`"import"`, `"media"`, `"property"`).
    AtKeyword(String),
    /// `#<value>` hash token (selector ID or `#hex` color literal).
    Hash(String),
    /// Quoted string literal (without outer quotes).
    Str(String),
    /// Unquoted `url(...)` payload.
    RawUrl(String),
    /// `<ident>(` function start (`"var"`, `"calc"`, `"url"`, `"oklch"`).
    Function(String),
    /// CSS identifier (`"color"`, `"--bg"`, `"cornflowerblue"`).
    Ident(String),
    /// Dimension `<number><unit>` (`num = "16"`, `unit = "px"`).
    Dimension {
        /// Numeric literal prefix.
        num: String,
        /// Lowercase unit suffix (`"px"`, `"rem"`, `"ms"`).
        unit: String,
    },
    /// Unitless number (`"0"`, `"-1"`, `"100"`).
    Number(String),
    /// `<number>%` percentage (`"100"`).
    Percentage(String),
    /// `:` colon.
    Colon,
    /// `;` semicolon.
    Semicolon,
    /// `,` comma.
    Comma,
    /// `!` bang (used for `!important`).
    Bang,
    /// Opening `{`, `(`, or `[`.
    Open(char),
    /// Closing `}`, `)`, or `]`.
    Close(char),
    /// Any other single-character delimiter.
    Delim(char),
}

/// Token paired with its 1-based CSS line number.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssToken {
    /// Token payload.
    pub kind: CssTokKind,
    /// 1-based line number in the CSS input.
    pub line: u32,
}

/// Output of tokenizing a CSS source string.
#[derive(Default)]
pub struct TokenizeOutput {
    /// Stream of CSS tokens.
    pub tokens: Vec<CssToken>,
    /// Unclosed comments, strings, or mismatched brackets (`GF000`).
    pub syntax_errors: Vec<SyntaxErrorNode>,
    /// Unjustified `stylelint-disable` / `biome-ignore` comment pragmas (`GF070`).
    pub bypasses: Vec<BypassFinding>,
}

pub(super) fn check_comment_bypass(body: &str, line: u32, out: &mut Vec<BypassFinding>) {
    let trimmed = body.trim();
    let lower = trimmed.to_ascii_lowercase();
    let directive = if lower.starts_with("stylelint-disable") {
        Some("/* stylelint-disable */")
    } else if lower.starts_with("biome-ignore") {
        Some("/* biome-ignore */")
    } else {
        None
    };
    if let Some(dir) = directive {
        let justified = trimmed
            .split_once("--")
            .is_some_and(|(_, r)| r.trim().chars().count() >= 10);
        if !justified {
            let span = Span::new(line, line).unwrap_or(Span::FIRST);
            out.push(BypassFinding {
                directive: String::from(dir),
                span,
            });
        }
    }
}

pub(super) fn verify_balanced_delimiters(tokens: &[CssToken], errors: &mut Vec<SyntaxErrorNode>) {
    let mut stack: Vec<(char, u32)> = Vec::new();
    for tok in tokens {
        match &tok.kind {
            CssTokKind::Open('{') => stack.push(('}', tok.line)),
            CssTokKind::Function(_) | CssTokKind::Open('(') => stack.push((')', tok.line)),
            CssTokKind::Open('[') => stack.push((']', tok.line)),
            CssTokKind::Close(ch) => match stack.pop() {
                Some((expected, _)) if expected == *ch => {}
                _ => errors.push(SyntaxErrorNode {
                    span: Span::new(tok.line, tok.line).unwrap_or(Span::FIRST),
                    message: format!("unexpected closing `{ch}` in CSS"),
                }),
            },
            _ => {}
        }
    }
    for (expected, open_line) in stack {
        errors.push(SyntaxErrorNode {
            span: Span::new(open_line, open_line).unwrap_or(Span::FIRST),
            message: format!("unclosed CSS delimiter (expected `{expected}`)"),
        });
    }
}

pub(super) const fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || !ch.is_ascii()
}

pub(super) fn starts_ident_chars(mut chars: Chars<'_>) -> bool {
    match chars.next() {
        Some('\\') => chars.next().is_some_and(|n| n != '\n'),
        Some(ch) => is_ident_char(ch),
        None => false,
    }
}

pub(super) fn starts_number_chars(mut chars: Chars<'_>) -> bool {
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some('0'..='9'), _, _)
            | (Some('.' | '+' | '-'), Some('0'..='9'), _)
            | (Some('+' | '-'), Some('.'), Some('0'..='9'))
    )
}

pub(super) const fn delim_tok_kind(ch: char) -> CssTokKind {
    match ch {
        ':' => CssTokKind::Colon,
        ';' => CssTokKind::Semicolon,
        ',' => CssTokKind::Comma,
        '!' => CssTokKind::Bang,
        '{' | '(' | '[' => CssTokKind::Open(ch),
        '}' | ')' | ']' => CssTokKind::Close(ch),
        _ => CssTokKind::Delim(ch),
    }
}
