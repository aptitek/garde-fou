//! Pure state-machine Markdown / MDX analyzer (`GF063` / `GF064`), ignoring fenced and
//! inline code blocks when hunting raw HTML, heading jumps, and broken/alt-less images.

use gf_core::{
    ContenusDocument, ContenusIssue, ContenusIssueKind, Language, RepoPath, Span, UastDocument,
};

use super::markdown_scan::{
    MdBlockMode, MdScanSession, detect_raw_html_tag, strip_inline_code_spans,
};
use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};

/// Safe-Rust Markdown (`.md`) and MDX (`.mdx`) extractor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownExtractor;

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
        let issues = {
            let mut session = MdScanSession {
                is_mdx,
                mode: MdBlockMode::Prose,
                prev_heading: 0,
                h1_count: 0,
                issues: Vec::new(),
                doc: &mut doc,
            };
            for (idx, raw_line) in source.lines().enumerate() {
                let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
                let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
                session.process_line(idx, raw_line, span);
            }
            session.issues
        };

        doc.contenus = Some(ContenusDocument { issues });
        Ok(doc)
    }
}

#[derive(Clone, Copy)]
struct FenceSpec {
    marker: char,
    len: usize,
}

fn parse_fence_marker(trimmed: &str) -> Option<FenceSpec> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let run_len = trimmed.chars().take_while(|&c| c == first).count();
    (run_len >= 3).then_some(FenceSpec {
        marker: first,
        len: run_len,
    })
}

impl MdScanSession<'_> {
    fn check_atx_heading(&mut self, trimmed: &str, span: Span) {
        let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
        if !(1..=6).contains(&hashes) || !trimmed[hashes..].starts_with(' ') {
            return;
        }
        let level = u8::try_from(hashes).unwrap_or(6);
        if level == 1 {
            self.h1_count = self.h1_count.saturating_add(1);
            if self.h1_count > 1 {
                self.issues.push(ContenusIssue {
                    kind: ContenusIssueKind::MultipleH1,
                    detail: String::from("multiple top-level `# H1` headings in document"),
                    span,
                });
            }
        }
        if self.prev_heading > 0 && level > self.prev_heading.saturating_add(1) {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::HeadingLevelJump,
                detail: format!("heading level jump `H{}` -> `H{level}`", self.prev_heading),
                span,
            });
        }
        self.prev_heading = level;
    }

    fn advance_fence_or_frontmatter(&mut self, idx: usize, trimmed: &str) -> bool {
        match self.mode {
            MdBlockMode::Prose if idx == 0 && trimmed == "---" => {
                self.mode = MdBlockMode::Frontmatter;
                true
            }
            MdBlockMode::Frontmatter => {
                if trimmed == "---" || trimmed == "..." {
                    self.mode = MdBlockMode::Prose;
                }
                true
            }
            MdBlockMode::CodeFence { marker, len } => {
                if let Some(close) = parse_fence_marker(trimmed)
                    && close.marker == marker
                    && close.len >= len
                    && trimmed[close.len..].trim().is_empty()
                {
                    self.mode = MdBlockMode::Prose;
                }
                true
            }
            MdBlockMode::Prose => {
                if let Some(open) = parse_fence_marker(trimmed) {
                    self.mode = MdBlockMode::CodeFence {
                        marker: open.marker,
                        len: open.len,
                    };
                    return true;
                }
                false
            }
        }
    }

    fn process_line(&mut self, idx: usize, raw_line: &str, span: Span) {
        if self.advance_fence_or_frontmatter(idx, raw_line.trim()) {
            return;
        }
        let prose = strip_inline_code_spans(raw_line);
        self.check_atx_heading(prose.trim_start(), span);
        self.extract_md_images(&prose, span);
        if let Some(tag) = detect_raw_html_tag(&prose, self.is_mdx) {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::RawHtml,
                detail: format!("raw HTML tag `<{tag}>` outside fenced/inline code"),
                span,
            });
        }
        if self.is_mdx {
            self.check_mdx_logic(prose.trim(), span);
        }
    }
}
