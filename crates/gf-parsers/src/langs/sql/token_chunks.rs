//! Top-level SQL token-chunk splitter preserving `$$...$$` dollar-quoted bodies
//! and `CREATE TRIGGER ... BEGIN ... END;` blocks.

use gf_core::Span;
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use super::dialect_detect::span_from_sql_range;

/// Returns `true` if `tok` is neither whitespace/comment trivia nor `EOF`.
#[must_use]
pub const fn is_non_trivia(tok: &Token) -> bool {
    !matches!(tok, Token::Whitespace(_) | Token::EOF)
}

/// Returns `Some(keyword)` when `tok` is a SQL word token (`sqlparser` assigns `Keyword::NoKeyword` to quoted identifiers).
#[must_use]
pub const fn kw_of(tok: &Token) -> Option<Keyword> {
    match tok {
        Token::Word(w) => Some(w.keyword),
        _ => None,
    }
}

/// Computes the 1-based [`Span`] enclosing the non-trivia tokens in `tokens`.
#[must_use]
pub fn chunk_span(tokens: &[TokenWithSpan]) -> Span {
    let first = tokens.iter().find(|t| is_non_trivia(&t.token));
    let last = tokens.iter().rev().find(|t| is_non_trivia(&t.token));
    match (first, last) {
        (Some(a), Some(b)) => span_from_sql_range(a.span.start.line, b.span.end.line),
        _ => Span::FIRST,
    }
}

struct SplitState {
    seen_create_trigger: bool,
    trigger_begin_depth: usize,
}

impl SplitState {
    const fn new() -> Self {
        Self {
            seen_create_trigger: false,
            trigger_begin_depth: 0,
        }
    }

    const fn observe_token(&mut self, tok: &Token) {
        if let Token::Word(w) = tok {
            match w.keyword {
                Keyword::TRIGGER => self.seen_create_trigger = true,
                Keyword::BEGIN if self.seen_create_trigger => {
                    self.seen_create_trigger = false;
                    self.trigger_begin_depth = 1;
                }
                Keyword::CASE if self.trigger_begin_depth != 0 => {
                    self.trigger_begin_depth = self.trigger_begin_depth.saturating_add(1);
                }
                Keyword::END => {
                    self.trigger_begin_depth = self.trigger_begin_depth.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    const fn at_top_level(&self) -> bool {
        self.trigger_begin_depth == 0
    }
}

/// Splits `tokens` into individual top-level statement token chunks at `;`.
#[must_use]
pub fn split_statement_chunks(tokens: Vec<TokenWithSpan>) -> Vec<Vec<TokenWithSpan>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut state = SplitState::new();
    for tws in tokens {
        if matches!(tws.token, Token::EOF) {
            break;
        }
        state.observe_token(&tws.token);
        if matches!(tws.token, Token::SemiColon) && state.at_top_level() {
            state.seen_create_trigger = false;
            if current
                .iter()
                .any(|t: &TokenWithSpan| is_non_trivia(&t.token))
            {
                chunks.push(core::mem::take(&mut current));
            } else {
                current.clear();
            }
        } else {
            current.push(tws);
        }
    }
    if current.iter().any(|t| is_non_trivia(&t.token)) {
        chunks.push(current);
    }
    chunks
}
