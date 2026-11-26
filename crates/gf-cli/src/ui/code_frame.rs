//! Pure Miette-inspired diagnostic code-frame renderer with numbered gutters,
//! clickable `path:line:col` links, `╰── ^^^` underlines, and binary/OOB fallbacks.

use super::ansi::{truncate_visible, visible_width};
use super::policy::ColorPolicy;
use super::theme::{Role, paint, paint_bold};

/// Pure in-memory view of a diagnosed repository file (loaded lazily by the imperative shell).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceFileView {
    /// Decoded UTF-8 source text.
    Text(String),
    /// Binary media or non-UTF8 asset whose snippet is intentionally omitted.
    BinaryOrOmitted,
    /// Virtual or unreadable path (e.g. workspace-level synthetic rule target).
    Unavailable,
}

/// Parameters describing a single diagnostic code snippet.
#[derive(Clone, Copy, Debug)]
pub struct CodeFrameSpec<'a> {
    /// Repository-relative path (`src/foo.rs`).
    pub path: &'a str,
    /// 1-indexed starting line of the diagnostic span.
    pub start_line: u32,
    /// 1-indexed ending line of the diagnostic span.
    pub end_line: u32,
    /// In-memory source representation for `path`.
    pub source: &'a SourceFileView,
}

#[derive(Clone, Copy)]
struct GutterLayout {
    gutter_w: usize,
    code_budget: usize,
    policy: ColorPolicy,
}

fn expand_tabs(line: &str) -> String {
    line.replace('\r', "").replace('\t', "    ")
}

fn measure_focus(expanded_line: &str) -> (usize, usize) {
    let trimmed_start = expanded_line.trim_start_matches(' ');
    let indent_cols = expanded_line.len().saturating_sub(trimmed_start.len());
    let trimmed = trimmed_start.trim_end_matches(' ');
    let highlight_cols = visible_width(trimmed).max(1);
    (indent_cols, highlight_cols)
}

fn format_link_header(path: &str, line: u32, col: usize, layout: GutterLayout) -> String {
    let lead = " ".repeat(layout.gutter_w + 1);
    let arrow = paint("╭─▸ ", Role::SurfaceBorder, layout.policy);
    let loc = paint_bold(&format!("{path}:{line}:{col}"), Role::Accent, layout.policy);
    format!("{lead}{arrow}{loc}")
}

fn format_fallback_frame(path: &str, line: u32, reason: &str, policy: ColorPolicy) -> Vec<String> {
    let gutter_w = line.max(1).to_string().len().max(2);
    let layout = GutterLayout {
        gutter_w,
        code_budget: 60,
        policy,
    };
    let header = format_link_header(path, line.max(1), 1, layout);
    let lead = " ".repeat(gutter_w + 1);
    let tail = paint("╰─ ", Role::SurfaceBorder, policy);
    let note = paint(reason, Role::Muted, policy);
    vec![header, format!("{lead}{tail}{note}")]
}

/// Renders a compact Miette-style code frame as a vector of lines ready for card embedding.
#[must_use]
pub fn render_code_frame(
    spec: &CodeFrameSpec<'_>,
    policy: ColorPolicy,
    max_width: usize,
) -> Vec<String> {
    let line_no = spec.start_line.max(1);
    let raw_text = match spec.source {
        SourceFileView::BinaryOrOmitted => {
            return format_fallback_frame(
                spec.path,
                line_no,
                "(extrait omis : fichier binaire ou asset)",
                policy,
            );
        }
        SourceFileView::Unavailable => {
            return format_fallback_frame(
                spec.path,
                line_no,
                "(extrait source indisponible)",
                policy,
            );
        }
        SourceFileView::Text(text) => text,
    };

    let all_lines: Vec<&str> = raw_text.lines().collect();
    let idx = (line_no as usize).saturating_sub(1);
    let Some(&target_raw) = all_lines.get(idx) else {
        let msg = format!(
            "(ligne {line_no} hors bornes — fichier de {} ligne(s))",
            all_lines.len()
        );
        return format_fallback_frame(spec.path, line_no, &msg, policy);
    };

    let gutter_w = spec.end_line.max(line_no).to_string().len().max(2);
    let layout = GutterLayout {
        gutter_w,
        code_budget: max_width.saturating_sub(gutter_w + 5).max(12),
        policy,
    };
    build_numbered_snippet(spec, &all_lines, target_raw, layout)
}

fn build_numbered_snippet(
    spec: &CodeFrameSpec<'_>,
    all_lines: &[&str],
    target_raw: &str,
    layout: GutterLayout,
) -> Vec<String> {
    let line_no = spec.start_line.max(1);
    let idx = (line_no as usize).saturating_sub(1);
    let expanded = expand_tabs(target_raw);
    let (indent_cols, span_cols) = measure_focus(&expanded);
    let gw = layout.gutter_w;

    let mut rows = Vec::with_capacity(5);
    rows.push(format_link_header(
        spec.path,
        line_no,
        indent_cols + 1,
        layout,
    ));

    if idx > 0
        && let Some(&prev_raw) = all_lines.get(idx - 1)
    {
        let prev_exp = truncate_visible(&expand_tabs(prev_raw), layout.code_budget);
        let num = paint(&format!("{:>gw$}", line_no - 1), Role::Muted, layout.policy);
        let sep = paint("│", Role::SurfaceBorder, layout.policy);
        rows.push(format!(
            "{num} {sep} {}",
            paint(&prev_exp, Role::Muted, layout.policy)
        ));
    }

    let clipped_target = truncate_visible(&expanded, layout.code_budget);
    let num = paint_bold(&format!("{line_no:>gw$}"), Role::Error, layout.policy);
    let sep = paint("│", Role::SurfaceBorder, layout.policy);
    rows.push(format!(
        "{num} {sep} {}",
        paint(&clipped_target, Role::CodeLine, layout.policy)
    ));
    rows.push(build_caret_line(indent_cols, span_cols, layout));

    if spec.end_line > line_no {
        let lead = " ".repeat(gw);
        let dot = paint("·", Role::SurfaceBorder, layout.policy);
        let span_note = paint(
            &format!(" ╰─ portée lignes {line_no}..={}", spec.end_line),
            Role::Muted,
            layout.policy,
        );
        rows.push(format!("{lead} {dot}{span_note}"));
    }
    rows
}

fn build_caret_line(indent_cols: usize, span_cols: usize, layout: GutterLayout) -> String {
    let lead = " ".repeat(layout.gutter_w);
    let dot = paint("·", Role::SurfaceBorder, layout.policy);
    let eff_indent = indent_cols.min(layout.code_budget.saturating_sub(6));
    let avail_carets = layout.code_budget.saturating_sub(eff_indent + 4).max(1);
    let caret_len = span_cols.min(avail_carets).max(1);
    let pointer = if eff_indent >= 4 {
        format!(
            "{}╰── {}",
            " ".repeat(eff_indent - 4),
            "^".repeat(caret_len)
        )
    } else {
        format!("{}{}", " ".repeat(eff_indent), "^".repeat(caret_len))
    };
    format!(
        "{lead} {dot} {}",
        paint_bold(&pointer, Role::Error, layout.policy)
    )
}
