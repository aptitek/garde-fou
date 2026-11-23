//! Pure Safe-Rust `Dockerfile` / `Containerfile` parser (directives, continuations, heredocs).

use gf_core::{DockerDocument, Language, RepoPath, Span, SyntaxErrorNode, UastDocument};

use super::dockerfile_instr::parse_instruction;
use crate::cst::line_metrics;
use crate::{LanguageExtractor, ParseError};

/// Pure Safe-Rust extractor for `Dockerfile` and `Containerfile` build recipes.
#[derive(Clone, Copy, Debug, Default)]
pub struct DockerfileExtractor;

impl LanguageExtractor for DockerfileExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::Dockerfile,
            total_lines,
            effective_lines,
        )?;
        let (logical_blocks, mut syntax_errors) = lex_logical_instructions(source);
        let mut docker = DockerDocument::default();

        for (span, raw_instr) in logical_blocks {
            if let Err(err_msg) = parse_instruction(&raw_instr, span, &mut docker) {
                syntax_errors.push(SyntaxErrorNode {
                    message: err_msg,
                    span,
                });
            }
        }

        doc.syntax_errors = syntax_errors;
        doc.docker = Some(docker);
        Ok(doc)
    }
}

fn parse_escape_directive(line: &str) -> Option<char> {
    let rest = line.trim().strip_prefix('#')?.trim();
    let (key, val) = rest.split_once('=')?;
    if !key.trim().eq_ignore_ascii_case("escape") {
        return None;
    }
    let ch = val.trim().chars().next()?;
    (ch == '\\' || ch == '`').then_some(ch)
}

fn detect_heredoc_delimiter(text: &str) -> Option<String> {
    let (_, after) = text.split_once("<<")?;
    let trimmed = after.strip_prefix('-').unwrap_or(after).trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|s| s.split_once('"').map(|(d, _)| d))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.split_once('\'').map(|(d, _)| d))
        })
        .unwrap_or_else(|| trimmed.split_whitespace().next().unwrap_or(""));
    let clean: String = unquoted
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    (!clean.is_empty()).then_some(clean)
}

fn strip_continuation(trimmed_end: &str, escape: char) -> Option<&str> {
    let without_esc = trimmed_end.strip_suffix(escape)?;
    if without_esc.ends_with(escape) {
        None
    } else {
        Some(without_esc)
    }
}

fn make_span(start_line: usize, end_line: usize) -> Span {
    let s = u32::try_from(start_line).unwrap_or(u32::MAX).max(1);
    let e = u32::try_from(end_line).unwrap_or(u32::MAX).max(s);
    Span::new(s, e).unwrap_or(Span::FIRST)
}

struct LexState {
    escape: char,
    in_directives: bool,
    buf: String,
    start_line: usize,
    end_line: usize,
    heredoc_eof: Option<String>,
}

impl LexState {
    const fn new() -> Self {
        Self {
            escape: '\\',
            in_directives: true,
            buf: String::new(),
            start_line: 1,
            end_line: 1,
            heredoc_eof: None,
        }
    }
}

fn consume_heredoc_line(
    line_no: usize,
    raw: &str,
    state: &mut LexState,
    blocks: &mut Vec<(Span, String)>,
) -> bool {
    let Some(delim) = state.heredoc_eof.as_deref() else {
        return false;
    };
    state.end_line = line_no;
    if raw.trim() == delim {
        state.heredoc_eof = None;
        blocks.push((
            make_span(state.start_line, state.end_line),
            core::mem::take(&mut state.buf),
        ));
    } else {
        state.buf.push('\n');
        state.buf.push_str(raw);
    }
    true
}

fn should_skip_line(line_no: usize, trimmed: &str, state: &mut LexState) -> bool {
    if !state.buf.is_empty() {
        if trimmed.starts_with('#') {
            state.end_line = line_no;
            return true;
        }
        return false;
    }
    if state.in_directives && trimmed.starts_with('#') {
        if let Some(esc) = parse_escape_directive(trimmed) {
            state.escape = esc;
        }
        return true;
    }
    state.in_directives = false;
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return true;
    }
    state.start_line = line_no;
    false
}

fn consume_standard_line(
    line_no: usize,
    raw: &str,
    state: &mut LexState,
    blocks: &mut Vec<(Span, String)>,
) {
    if should_skip_line(line_no, raw.trim(), state) {
        return;
    }
    state.end_line = line_no;
    let trimmed_end = raw.trim_end();
    if let Some(prefix) = strip_continuation(trimmed_end, state.escape) {
        state.buf.push_str(prefix.trim_end());
        state.buf.push(' ');
        return;
    }

    state.buf.push_str(trimmed_end);
    if let Some(delim) = detect_heredoc_delimiter(&state.buf) {
        state.heredoc_eof = Some(delim);
        return;
    }
    blocks.push((
        make_span(state.start_line, state.end_line),
        core::mem::take(&mut state.buf),
    ));
}

fn lex_logical_instructions(source: &str) -> (Vec<(Span, String)>, Vec<SyntaxErrorNode>) {
    let mut state = LexState::new();
    let mut blocks = Vec::new();
    let mut errors = Vec::new();

    for (idx, raw_line) in source.lines().enumerate() {
        let raw = raw_line.trim_end_matches('\r');
        let line_no = idx.saturating_add(1);
        if !consume_heredoc_line(line_no, raw, &mut state, &mut blocks) {
            consume_standard_line(line_no, raw, &mut state, &mut blocks);
        }
    }

    if state.heredoc_eof.is_some() {
        errors.push(SyntaxErrorNode {
            message: String::from("unterminated Dockerfile heredoc"),
            span: make_span(state.start_line, state.end_line),
        });
    } else if !state.buf.trim().is_empty() {
        blocks.push((make_span(state.start_line, state.end_line), state.buf));
    }
    (blocks, errors)
}
