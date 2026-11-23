//! Pure state-machine Markdown / MDX analyzer (`GF063` / `GF064`), ignoring fenced and
//! inline code blocks when hunting raw HTML, heading jumps, and broken/alt-less images.

use gf_core::{
    ContenusDocument, ContenusIssue, ContenusIssueKind, Language, RepoPath, Span, UastDocument,
};

use super::markdown_scan::{
    check_mdx_logic, detect_raw_html_tag, extract_md_images, strip_inline_code_spans,
};
use crate::cst::line_metrics;
use crate::{LanguageExtractor, ParseError};

/// Safe-Rust Markdown (`.md`) and MDX (`.mdx`) extractor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownExtractor;

struct MdState {
    is_mdx: bool,
    in_frontmatter: bool,
    fence: Option<(char, usize)>,
    prev_heading: u8,
    h1_count: u32,
}

impl LanguageExtractor for MarkdownExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::Markdown,
            total_lines,
            effective_lines,
        )?;
        let is_mdx = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"));
        let mut state = MdState {
            is_mdx,
            in_frontmatter: false,
            fence: None,
            prev_heading: 0,
            h1_count: 0,
        };
        let mut issues = Vec::new();

        for (idx, raw_line) in source.lines().enumerate() {
            let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
            let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
            process_md_line(idx, raw_line, span, (&mut state, &mut issues, &mut doc));
        }

        doc.contenus = Some(ContenusDocument { issues });
        Ok(doc)
    }
}

fn parse_fence_marker(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let run_len = trimmed.chars().take_while(|&c| c == first).count();
    (run_len >= 3).then_some((first, run_len))
}

fn check_atx_heading(
    trimmed: &str,
    span: Span,
    state: &mut MdState,
    issues: &mut Vec<ContenusIssue>,
) {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) || !trimmed[hashes..].starts_with(' ') {
        return;
    }
    let level = u8::try_from(hashes).unwrap_or(6);
    if level == 1 {
        state.h1_count = state.h1_count.saturating_add(1);
        if state.h1_count > 1 {
            issues.push(ContenusIssue {
                kind: ContenusIssueKind::MultipleH1,
                detail: String::from("multiple top-level `# H1` headings in document"),
                span,
            });
        }
    }
    if state.prev_heading > 0 && level > state.prev_heading.saturating_add(1) {
        issues.push(ContenusIssue {
            kind: ContenusIssueKind::HeadingLevelJump,
            detail: format!("heading level jump `H{}` -> `H{level}`", state.prev_heading),
            span,
        });
    }
    state.prev_heading = level;
}

fn advance_fence_or_frontmatter(idx: usize, trimmed: &str, state: &mut MdState) -> bool {
    if idx == 0 && trimmed == "---" {
        state.in_frontmatter = true;
        return true;
    }
    if state.in_frontmatter {
        if trimmed == "---" || trimmed == "..." {
            state.in_frontmatter = false;
        }
        return true;
    }
    if let Some((open_ch, open_len)) = state.fence {
        if let Some((ch, len)) = parse_fence_marker(trimmed)
            && ch == open_ch
            && len >= open_len
            && trimmed[len..].trim().is_empty()
        {
            state.fence = None;
        }
        return true;
    }
    if let Some(marker) = parse_fence_marker(trimmed) {
        state.fence = Some(marker);
        return true;
    }
    false
}

fn process_md_line(
    idx: usize,
    raw_line: &str,
    span: Span,
    ctx: (&mut MdState, &mut Vec<ContenusIssue>, &mut UastDocument),
) {
    let (state, issues, doc) = ctx;
    if advance_fence_or_frontmatter(idx, raw_line.trim(), state) {
        return;
    }

    let prose = strip_inline_code_spans(raw_line);
    check_atx_heading(prose.trim_start(), span, state, issues);
    extract_md_images(&prose, span, issues, doc);
    if let Some(tag) = detect_raw_html_tag(&prose, state.is_mdx) {
        issues.push(ContenusIssue {
            kind: ContenusIssueKind::RawHtml,
            detail: format!("raw HTML tag `<{tag}>` outside fenced/inline code"),
            span,
        });
    }
    if state.is_mdx {
        check_mdx_logic(prose.trim(), span, issues);
    }
}
