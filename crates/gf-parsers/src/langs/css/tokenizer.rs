//! Pure Safe-Rust W3C CSS Syntax Level 3 lexer built over `std::str::Chars` (zero byte-offset arithmetic).

use std::str::Chars;

use gf_core::{Span, SyntaxErrorNode};

pub use super::css_tokens::{CssTokKind, CssToken, TokenizeOutput};
use super::css_tokens::{
    check_comment_bypass, delim_tok_kind, is_ident_char, starts_ident_chars, starts_number_chars,
    verify_balanced_delimiters,
};

struct Lexer<'a> {
    chars: Chars<'a>,
    line: u32,
    out: TokenizeOutput,
}

impl Lexer<'_> {
    fn peek0(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn peek1(&self) -> Option<char> {
        let mut c = self.chars.clone();
        c.next();
        c.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        if ch == '\n' {
            self.line = self.line.saturating_add(1);
        }
        Some(ch)
    }

    fn push_tok(&mut self, kind: CssTokKind, line: u32) {
        self.out.tokens.push(CssToken { kind, line });
    }

    fn push_err(&mut self, line: u32, msg: &str) {
        self.out.syntax_errors.push(SyntaxErrorNode {
            span: Span::new(line, line).unwrap_or(Span::FIRST),
            message: String::from(msg),
        });
    }

    fn read_ident(&mut self) -> String {
        let mut buf = String::new();
        while let Some(ch) = self.peek0() {
            if ch == '\\' && self.peek1().is_some_and(|n| n != '\n') {
                self.bump();
                buf.push('\\');
                if let Some(esc) = self.bump() {
                    buf.push(esc);
                }
            } else if is_ident_char(ch) {
                self.bump();
                buf.push(ch);
            } else {
                break;
            }
        }
        buf
    }

    fn lex_number_or_dimension(&mut self) {
        let line = self.line;
        let mut num = String::new();
        if let Some(sign @ ('+' | '-')) = self.peek0() {
            self.bump();
            num.push(sign);
        }
        while let Some(d) = self.peek0().filter(char::is_ascii_digit) {
            self.bump();
            num.push(d);
        }
        if self.peek0() == Some('.') && self.peek1().is_some_and(|d| d.is_ascii_digit()) {
            self.bump();
            num.push('.');
            while let Some(d) = self.peek0().filter(char::is_ascii_digit) {
                self.bump();
                num.push(d);
            }
        }
        let kind = if self.peek0() == Some('%') {
            self.bump();
            CssTokKind::Percentage(num)
        } else if starts_ident_chars(self.chars.clone()) {
            CssTokKind::Dimension {
                num,
                unit: self.read_ident().to_ascii_lowercase(),
            }
        } else {
            CssTokKind::Number(num)
        };
        self.push_tok(kind, line);
    }

    fn try_lex_unquoted_url(&mut self) -> Option<String> {
        while self
            .peek0()
            .is_some_and(|c| c.is_ascii_whitespace() && c != '\n')
        {
            self.bump();
        }
        if matches!(self.peek0(), Some('"' | '\'' | ')')) {
            return None;
        }
        let mut raw = String::new();
        while let Some(ch) = self.peek0() {
            if ch == ')' || ch.is_ascii_whitespace() {
                break;
            }
            self.bump();
            raw.push(ch);
        }
        Some(raw)
    }

    fn lex_ident_or_function(&mut self) {
        let line = self.line;
        let ident = self.read_ident();
        if self.peek0() != Some('(') {
            self.push_tok(CssTokKind::Ident(ident), line);
            return;
        }
        self.bump();
        let raw_url = ident
            .eq_ignore_ascii_case("url")
            .then(|| self.try_lex_unquoted_url())
            .flatten();
        self.push_tok(CssTokKind::Function(ident), line);
        if let Some(raw) = raw_url {
            self.push_tok(CssTokKind::RawUrl(raw), line);
        }
    }
}

fn lex_block_comment(lx: &mut Lexer<'_>) {
    let start_line = lx.line;
    lx.bump();
    lx.bump();
    let mut body = String::new();
    while let Some(ch) = lx.bump() {
        if ch == '*' && lx.peek0() == Some('/') {
            lx.bump();
            check_comment_bypass(&body, start_line, &mut lx.out.bypasses);
            return;
        }
        body.push(ch);
    }
    lx.push_err(start_line, "unclosed CSS block comment `/*`");
}

fn lex_string(lx: &mut Lexer<'_>, quote: char) {
    let start_line = lx.line;
    lx.bump();
    let mut buf = String::new();
    while let Some(ch) = lx.peek0() {
        if ch == quote {
            lx.bump();
            lx.push_tok(CssTokKind::Str(buf), start_line);
            return;
        }
        if ch == '\n' {
            lx.push_err(start_line, "unclosed CSS string literal before newline");
            return;
        }
        lx.bump();
        if ch == '\\' {
            if let Some(esc) = lx.bump()
                && esc != '\n'
            {
                buf.push(esc);
            }
        } else {
            buf.push(ch);
        }
    }
    lx.push_err(start_line, "unclosed CSS string literal at EOF");
}

fn try_lex_comment_or_space(lx: &mut Lexer<'_>, ch: char) -> bool {
    if ch.is_ascii_whitespace() {
        lx.bump();
        return true;
    }
    if ch == '/' && lx.peek1() == Some('*') {
        lex_block_comment(lx);
        return true;
    }
    if ch == '/' && lx.peek1() == Some('/') {
        while lx.bump().is_some_and(|c| c != '\n') {}
        return true;
    }
    false
}

fn step_single_token(lx: &mut Lexer<'_>, ch: char) {
    if try_lex_comment_or_space(lx, ch) {
        return;
    }
    let line = lx.line;
    let mut after_first = lx.chars.clone();
    after_first.next();
    if ch == '"' || ch == '\'' {
        lex_string(lx, ch);
    } else if starts_number_chars(lx.chars.clone()) {
        lx.lex_number_or_dimension();
    } else if ch == '@' && starts_ident_chars(after_first.clone()) {
        lx.bump();
        let name = lx.read_ident();
        lx.push_tok(CssTokKind::AtKeyword(name), line);
    } else if ch == '#' && starts_ident_chars(after_first) {
        lx.bump();
        let hash = lx.read_ident();
        lx.push_tok(CssTokKind::Hash(hash), line);
    } else if starts_ident_chars(lx.chars.clone()) {
        lx.lex_ident_or_function();
    } else {
        lx.bump();
        lx.push_tok(delim_tok_kind(ch), line);
    }
}

/// Tokenizes `source` according to W3C CSS Syntax Level 3 and verifies balanced delimiters.
#[must_use]
pub fn tokenize_css(source: &str) -> TokenizeOutput {
    let mut lx = Lexer {
        chars: source.chars(),
        line: 1,
        out: TokenizeOutput::default(),
    };
    while let Some(ch) = lx.peek0() {
        step_single_token(&mut lx, ch);
    }
    verify_balanced_delimiters(&lx.out.tokens, &mut lx.out.syntax_errors);
    lx.out
}
