//! Native Quarto (`.qmd` & `_quarto.yml`) extractor.
//!
//! Verifies Pandoc fenced divs `:::`, `{{< include >}}`/`{{< embed >}}` shortcodes,
//! `@fig-*`/`@tbl-*`/`@sec-*`/`@eq-*` cross-references, `#|` options, and executable code cells.

pub mod config_audit;
mod qmd_cells;
mod qmd_prose;

use std::collections::BTreeSet;

use gf_core::{
    AcademicSummary, LangRuleFinding, Language, QuartoViolation, RepoPath, Span, SqlDialectKind,
    UastDocument, WorkspaceRefItem,
};

use crate::cst::line_metrics;
use crate::langs::yaml::events_tree::parse_yaml_stream;
use crate::types::{LanguageExtractor, ParseError};
use qmd_cells::{QmdCellLang, QmdCellSpec, analyze_qmd_cell, parse_qmd_executable_lang};
use qmd_prose::{
    extract_crossrefs_outside_inline_code, extract_inline_anchors, extract_shortcodes,
    update_fenced_div_stack,
};

/// Extractor for Quarto Markdown (`.qmd`) documents.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuartoExtractor;

fn parse_frontmatter_end(lines: &[&str], doc: &mut UastDocument) -> usize {
    if lines.first().map(|l| l.trim()) != Some("---") {
        return 0;
    }
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if matches!(line.trim(), "---" | "...") {
            doc.syntax_errors
                .extend(parse_yaml_stream(&lines[1..idx].join("\n")).syntax_errors);
            return idx + 1;
        }
    }
    0
}

struct ActiveFence<'a> {
    marker: char,
    len: usize,
    exec_lang: Option<QmdCellLang>,
    body_start_line: u32,
    body_lines: Vec<&'a str>,
}

struct FenceOpen<'a> {
    marker: char,
    len: usize,
    info: &'a str,
}

fn fence_open(trimmed: &str) -> Option<FenceOpen<'_>> {
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|&c| c == ch).count();
    (len >= 3).then_some(FenceOpen {
        marker: ch,
        len,
        info: &trimmed[len..],
    })
}

#[derive(Default)]
struct QmdWalkAcc {
    div_stack: Vec<u32>,
    defined_labels: BTreeSet<String>,
    crossrefs: Vec<(String, Span)>,
    shortcode_refs: Vec<WorkspaceRefItem>,
}

fn finalize_qmd_acc(acc: QmdWalkAcc, doc: &mut UastDocument) {
    for open_line in acc.div_stack {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            QuartoViolation::UnclosedDiv,
            Span::new(open_line, open_line).unwrap_or(Span::FIRST),
            "unclosed Pandoc fenced div `:::` opened here",
        ));
    }
    for (ident, span) in acc.crossrefs {
        if !acc.defined_labels.contains(&ident) {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                QuartoViolation::BrokenCrossref,
                span,
                format!("Quarto cross-reference `@{ident}` has no matching `#| label:` or `{{#{ident}}}`"),
            ));
        }
    }
    if !acc.shortcode_refs.is_empty() {
        doc.academic = Some(AcademicSummary {
            quarto_shortcode_refs: acc.shortcode_refs,
            ..AcademicSummary::default()
        });
    }
}

struct QmdRunCtx<'a> {
    path: &'a RepoPath,
    sql_dialect: Option<SqlDialectKind>,
    acc: QmdWalkAcc,
    fence: Option<ActiveFence<'a>>,
}

impl<'a> QmdRunCtx<'a> {
    fn close_active_fence(&mut self, doc: &mut UastDocument) -> Result<(), ParseError> {
        if let Some(fin) = self.fence.take()
            && let Some(lang) = fin.exec_lang
        {
            let spec = QmdCellSpec {
                path: self.path,
                lang,
                body_start_line: fin.body_start_line,
                raw_lines: &fin.body_lines,
                sql_dialect: self.sql_dialect,
            };
            analyze_qmd_cell(&spec, &mut self.acc.defined_labels, doc)?;
        }
        Ok(())
    }

    fn step_line(
        &mut self,
        raw: &'a str,
        lno: u32,
        doc: &mut UastDocument,
    ) -> Result<(), ParseError> {
        let trimmed = raw.trim();
        if let Some(active) = self.fence.as_mut() {
            let is_close = fence_open(trimmed).is_some_and(|open| {
                open.marker == active.marker
                    && open.len >= active.len
                    && open.info.trim().is_empty()
            });
            if !is_close {
                active.body_lines.push(raw);
                return Ok(());
            }
            return self.close_active_fence(doc);
        }
        if let Some(open) = fence_open(trimmed) {
            self.fence = Some(ActiveFence {
                marker: open.marker,
                len: open.len,
                exec_lang: parse_qmd_executable_lang(open.info),
                body_start_line: lno.saturating_add(1),
                body_lines: Vec::new(),
            });
            return Ok(());
        }
        update_fenced_div_stack(
            trimmed,
            lno,
            &mut self.acc.div_stack,
            &mut doc.lang_rule_findings,
        );
        extract_inline_anchors(raw, &mut self.acc.defined_labels);
        extract_shortcodes(raw, lno, &mut self.acc.shortcode_refs);
        extract_crossrefs_outside_inline_code(raw, lno, &mut self.acc.crossrefs);
        Ok(())
    }
}

impl QuartoExtractor {
    /// Extracts a Quarto `.qmd` [`UastDocument`] with an optional workspace SQL dialect hint.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if inner Tree-sitter grammar initialization or span construction fails.
    pub fn extract_with_sql_dialect(
        path: &RepoPath,
        source: &str,
        sql_dialect: Option<SqlDialectKind>,
    ) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Quarto, total_lines, effective_lines)?;
        let lines: Vec<&str> = source.lines().collect();
        let start_idx = parse_frontmatter_end(&lines, &mut doc);
        let mut ctx = QmdRunCtx {
            path,
            sql_dialect,
            acc: QmdWalkAcc::default(),
            fence: None,
        };
        for (idx, &raw_line) in lines.iter().enumerate().skip(start_idx) {
            let lno = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            ctx.step_line(raw_line, lno, &mut doc)?;
        }
        finalize_qmd_acc(ctx.acc, &mut doc);
        Ok(doc)
    }
}

impl LanguageExtractor for QuartoExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        Self::extract_with_sql_dialect(path, source, None)
    }
}
