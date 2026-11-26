//! Quarto `.qmd` executable code cell extractor (`{python}`, `{bash}`, `{sh}`, `{sql}`, `{ts}`),
//! `#| ...` YAML cell option validator, and multi-AST bridge.

use std::collections::BTreeSet;

use gf_core::{
    DomainKind, LangRuleFinding, QuartoViolation, RepoPath, Span, SqlDialectKind, SyntaxErrorNode,
    UastDocument,
};

use crate::langs::python::PythonExtractor;
use crate::langs::shell::ShellExtractor;
use crate::langs::sql::extract_sql_with_dialect_hint;
use crate::langs::ts::{TsDialect, TypeScriptExtractor};
use crate::langs::yaml::events_tree::parse_yaml_stream;
use crate::types::{LanguageExtractor, ParseError};

/// Executable cell language parsed from ```` ```{lang} ```` in `.qmd`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QmdCellLang {
    Python,
    Bash,
    PosixSh,
    Sql,
    TypeScript,
}

/// Parses an executable Quarto fence header (returns `None` for passive ```` ```{{python}} ```` or `{ojs}`).
#[must_use]
pub fn parse_qmd_executable_lang(info: &str) -> Option<QmdCellLang> {
    let trimmed = info.trim();
    if trimmed.starts_with("{{") {
        return None;
    }
    let inner = trimmed.strip_prefix('{')?.split('}').next()?.trim();
    let lang = inner
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .next()
        .unwrap_or("");
    match lang.to_ascii_lowercase().as_str() {
        "python" | "py" => Some(QmdCellLang::Python),
        "bash" => Some(QmdCellLang::Bash),
        "sh" => Some(QmdCellLang::PosixSh),
        "sql" => Some(QmdCellLang::Sql),
        "ts" | "typescript" => Some(QmdCellLang::TypeScript),
        _ => None,
    }
}

fn shift_span(base_line: u32, inner: Span) -> Span {
    let start = base_line.saturating_add(inner.start_line().saturating_sub(1));
    let end = base_line.saturating_add(inner.end_line().saturating_sub(1));
    Span::new(start.max(1), end.max(start).max(1)).unwrap_or(Span::FIRST)
}

fn validate_cell_yaml_options(
    yaml_lines: &[(u32, &str)],
    labels: &mut BTreeSet<String>,
    findings: &mut Vec<LangRuleFinding>,
) {
    if yaml_lines.is_empty() {
        return;
    }
    let first_line = yaml_lines[0].0;
    let last_line = yaml_lines.last().map_or(first_line, |&(l, _)| l);
    let mut joined = String::new();
    for &(_, text) in yaml_lines {
        if let Some(val) = text.trim().strip_prefix("label:") {
            let clean = val.trim().trim_matches(|c| c == '"' || c == '\'');
            if !clean.is_empty() {
                labels.insert(String::from(clean));
            }
        }
        joined.push_str(text);
        joined.push('\n');
    }
    let stream = parse_yaml_stream(&joined);
    if let Some(err) = stream.syntax_errors.first() {
        let span = Span::new(first_line, last_line).unwrap_or(Span::FIRST);
        findings.push(LangRuleFinding::new(
            QuartoViolation::InvalidCellOptionYaml,
            span,
            format!("invalid YAML in `#|` Quarto cell options: {}", err.message),
        ));
    }
}

fn merge_sub_doc(
    base_line: u32,
    sub: UastDocument,
    target: &mut UastDocument,
    keep_callables: bool,
) {
    for err in sub.syntax_errors {
        target.syntax_errors.push(SyntaxErrorNode {
            span: shift_span(base_line, err.span),
            message: err.message,
        });
    }
    for mut finding in sub.lang_rule_findings {
        finding.span = shift_span(base_line, finding.span);
        target.lang_rule_findings.push(finding);
    }
    if keep_callables {
        for mut c in sub.callables {
            c.span = shift_span(base_line, c.span);
            target.callables.push(c);
        }
    }
}

/// Parsed executable `.qmd` cell payload passed to [`analyze_qmd_cell`].
pub struct QmdCellSpec<'a> {
    /// Repository path of the `.qmd` file.
    pub path: &'a RepoPath,
    /// Target executable cell language.
    pub lang: QmdCellLang,
    /// 1-based start line of the cell body inside the `.qmd` file.
    pub body_start_line: u32,
    /// Raw lines inside the fenced block.
    pub raw_lines: &'a [&'a str],
    /// Optional workspace SQL dialect hint.
    pub sql_dialect: Option<SqlDialectKind>,
}

/// Validates `#|` YAML options and bridges a `.qmd` executable cell body to its native AST extractor.
pub fn analyze_qmd_cell(
    spec: &QmdCellSpec<'_>,
    labels: &mut BTreeSet<String>,
    target: &mut UastDocument,
) -> Result<(), ParseError> {
    let mut opt_lines = Vec::new();
    let mut code_start_idx = 0;
    for (idx, &line) in spec.raw_lines.iter().enumerate() {
        if let Some(rest) = line.trim_start().strip_prefix("#|") {
            let lno = spec
                .body_start_line
                .saturating_add(u32::try_from(idx).unwrap_or(0));
            opt_lines.push((lno, rest.strip_prefix(' ').unwrap_or(rest)));
            code_start_idx = idx + 1;
        } else {
            break;
        }
    }
    validate_cell_yaml_options(&opt_lines, labels, &mut target.lang_rule_findings);

    let code_body = spec.raw_lines[code_start_idx..].join("\n");
    if code_body.trim().is_empty() {
        return Ok(());
    }
    let code_base = spec
        .body_start_line
        .saturating_add(u32::try_from(code_start_idx).unwrap_or(0));
    bridge_qmd_code(spec, code_base, &code_body, target)
}

fn bridge_qmd_code(
    spec: &QmdCellSpec<'_>,
    code_base: u32,
    code_body: &str,
    target: &mut UastDocument,
) -> Result<(), ParseError> {
    match spec.lang {
        QmdCellLang::Python => {
            let sub =
                PythonExtractor::extract_in_domain(spec.path, code_body, DomainKind::Contenus)?;
            merge_sub_doc(code_base, sub, target, true);
        }
        QmdCellLang::Bash => {
            let wrapped = format!("#!/usr/bin/env bash\nset -euo pipefail\n{code_body}\n");
            let sub = ShellExtractor.extract(spec.path, &wrapped)?;
            merge_sub_doc(code_base.saturating_sub(2).max(1), sub, target, false);
        }
        QmdCellLang::PosixSh => {
            let wrapped = format!("#!/bin/sh\nset -eu\n{code_body}\n");
            let sub = ShellExtractor.extract(spec.path, &wrapped)?;
            merge_sub_doc(code_base.saturating_sub(2).max(1), sub, target, false);
        }
        QmdCellLang::Sql => {
            let sub = extract_sql_with_dialect_hint(spec.path, code_body, spec.sql_dialect)?;
            merge_sub_doc(code_base, sub, target, false);
        }
        QmdCellLang::TypeScript => {
            let sub = TypeScriptExtractor::new(TsDialect::TypeScript).extract_in_domain(
                spec.path,
                code_body,
                DomainKind::Contenus,
            )?;
            merge_sub_doc(code_base, sub, target, true);
        }
    }
    Ok(())
}
