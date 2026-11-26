//! `IPython` magic neutralization and multi-AST cell bridging (`Python`, `Shell`, `SQL`, `Markdown`)
//! for Jupyter Notebooks (`.ipynb`).

use gf_core::{DomainKind, RepoPath, Span, SqlDialectKind, SyntaxErrorNode, UastDocument};

use super::ipynb_parser::ParsedCell;
use crate::cst::line_metrics;
use crate::langs::markdown::MarkdownExtractor;
use crate::langs::python::PythonExtractor;
use crate::langs::shell::ShellExtractor;
use crate::langs::sql::extract_sql_with_dialect_hint;
use crate::types::{LanguageExtractor, ParseError};

const PYTHON_PRESERVING_CELL_MAGICS: &[&str] = &[
    "capture", "debug", "ipytest", "prun", "pypy", "python", "python3", "time", "timeit",
];

enum CellBridgeKind {
    Python,
    Shell,
    Sql,
    Skip,
}

fn classify_code_cell(source: &str) -> CellBridgeKind {
    let first = source
        .lines()
        .find(|l| !l.trim().is_empty())
        .map_or("", str::trim);
    let Some(rest) = first.strip_prefix("%%") else {
        return CellBridgeKind::Python;
    };
    let cmd = rest.split_whitespace().next().unwrap_or("");
    match cmd {
        "bash" | "sh" => CellBridgeKind::Shell,
        "sql" => CellBridgeKind::Sql,
        other if PYTHON_PRESERVING_CELL_MAGICS.contains(&other) => CellBridgeKind::Python,
        _ => CellBridgeKind::Skip,
    }
}

fn is_ipython_magic_or_help(trimmed: &str) -> bool {
    trimmed.starts_with('%')
        || trimmed.starts_with('!')
        || trimmed.starts_with('?')
        || (trimmed.ends_with('?') && !trimmed.contains('"') && !trimmed.contains('\''))
}

/// Neutralizes `IPython` magics (`%...`, `%%time`, `!cmd`, `?obj`) into `# magic` comments
/// preserving exact line count so `tree-sitter-python` never emits false `GF000` errors.
#[must_use]
pub fn neutralize_ipython_source(source: &str) -> (String, Vec<(u32, String)>) {
    let mut out = String::with_capacity(source.len());
    let mut shell_escapes = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if is_ipython_magic_or_help(trimmed) {
            if let Some(cmd) = trimmed.strip_prefix('!') {
                let lno = u32::try_from(idx + 1).unwrap_or(1);
                shell_escapes.push((lno, String::from(cmd.trim())));
            }
            let indent_len = line.len().saturating_sub(trimmed.len());
            out.push_str(&line[..indent_len]);
            out.push_str("# ipython-magic");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    (out, shell_escapes)
}

fn shift_span(cell_json_line: u32, inner: Span) -> Span {
    let start = cell_json_line.saturating_add(inner.start_line().saturating_sub(1));
    let end = cell_json_line.saturating_add(inner.end_line().saturating_sub(1));
    Span::new(start.max(1), end.max(start).max(1)).unwrap_or(Span::FIRST)
}

fn merge_cell_findings(
    cell: &ParsedCell,
    sub: UastDocument,
    target: &mut UastDocument,
    keep_callables: bool,
) {
    for err in sub.syntax_errors {
        target.syntax_errors.push(SyntaxErrorNode {
            span: shift_span(cell.json_line, err.span),
            message: format!("[cell #{}] {}", cell.cell_index, err.message),
        });
    }
    for mut finding in sub.lang_rule_findings {
        finding.span = shift_span(cell.json_line, finding.span);
        finding.message = format!("[cell #{}] {}", cell.cell_index, finding.message);
        target.lang_rule_findings.push(finding);
    }
    if keep_callables {
        for mut callable in sub.callables {
            callable.span = shift_span(cell.json_line, callable.span);
            callable.name = format!("[cell #{}] {}", cell.cell_index, callable.name);
            target.callables.push(callable);
        }
    }
}

fn bridge_shell_cell(
    path: &RepoPath,
    cell: &ParsedCell,
    target: &mut UastDocument,
) -> Result<(), ParseError> {
    let body = cell.source.lines().skip(1).collect::<Vec<_>>().join("\n");
    let wrapped = format!("#!/usr/bin/env bash\nset -euo pipefail\n{body}\n");
    let sub = ShellExtractor.extract(path, &wrapped)?;
    merge_cell_findings(cell, sub, target, false);
    Ok(())
}

fn bridge_sql_cell(
    path: &RepoPath,
    cell: &ParsedCell,
    sql_dialect: Option<SqlDialectKind>,
    target: &mut UastDocument,
) -> Result<(), ParseError> {
    let body = cell.source.lines().skip(1).collect::<Vec<_>>().join("\n");
    let sub = extract_sql_with_dialect_hint(path, &body, sql_dialect)?;
    merge_cell_findings(cell, sub, target, false);
    Ok(())
}

fn bridge_python_cell(
    path: &RepoPath,
    cell: &ParsedCell,
    target: &mut UastDocument,
) -> Result<(), ParseError> {
    let (neutralized, shell_escapes) = neutralize_ipython_source(&cell.source);
    let sub = PythonExtractor::extract_in_domain(path, &neutralized, DomainKind::Contenus)?;
    merge_cell_findings(cell, sub, target, true);
    for (rel_line, cmd) in shell_escapes {
        let wrapped = format!("#!/bin/sh\nset -eu\n{cmd}\n");
        let sh_doc = ShellExtractor.extract(path, &wrapped)?;
        for mut finding in sh_doc.lang_rule_findings {
            if finding.code() == "SH-CURL-PIPE-UNSAFE" {
                let line = cell.json_line.saturating_add(rel_line.saturating_sub(1));
                finding.span = Span::new(line.max(1), line.max(1)).unwrap_or(Span::FIRST);
                finding.message = format!("[cell #{}] {}", cell.cell_index, finding.message);
                target.lang_rule_findings.push(finding);
            }
        }
    }
    Ok(())
}

/// Bridges all parsed notebook cells into Python, Shell, SQL, and Markdown AST analyzers,
/// returning `(cell_index_1based, effective_lines, span)` metrics for each code cell.
///
/// # Errors
///
/// Returns [`ParseError`] if inner Tree-sitter grammar initialization or span construction fails.
pub fn bridge_notebook_cells(
    path: &RepoPath,
    cells: &[ParsedCell],
    sql_dialect: Option<SqlDialectKind>,
    target: &mut UastDocument,
) -> Result<Vec<(u32, u32, Span)>, ParseError> {
    let mut cell_lines = Vec::new();
    let mut combined_md = String::new();
    for cell in cells {
        match cell.cell_type.as_str() {
            "markdown" => {
                combined_md.push_str(&cell.source);
                combined_md.push_str("\n\n");
            }
            "code" => {
                let (_, eff) = line_metrics(&cell.source);
                let span = Span::new(cell.json_line, cell.json_line).unwrap_or(Span::FIRST);
                cell_lines.push((cell.cell_index, eff, span));
                match classify_code_cell(&cell.source) {
                    CellBridgeKind::Python => bridge_python_cell(path, cell, target)?,
                    CellBridgeKind::Shell => bridge_shell_cell(path, cell, target)?,
                    CellBridgeKind::Sql => bridge_sql_cell(path, cell, sql_dialect, target)?,
                    CellBridgeKind::Skip => {}
                }
            }
            _ => {}
        }
    }
    if !combined_md.trim().is_empty() {
        let md_doc = MarkdownExtractor.extract(path, &combined_md)?;
        target.asset_refs.extend(md_doc.asset_refs);
        target.contenus = md_doc.contenus;
    }
    Ok(cell_lines)
}
