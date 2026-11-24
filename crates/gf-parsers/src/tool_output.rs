//! Pure I/O-free JSON and NDJSON output parsers for `ruff`, `mypy`, and `pyright` (`GF082`).

mod path_norm;
mod tsc_parser;

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::{Deserialize, de::DeserializeOwned};

pub use path_norm::ToolParseError;
use path_norm::normalize_tool_path;
pub(crate) use path_norm::normalize_tool_path_with_fallback;
pub(crate) use tsc_parser::byte_offset_to_line_col;
pub use tsc_parser::parse_tsc_output;

#[derive(Deserialize)]
struct RuffLocation {
    row: u32,
    column: u32,
}

#[derive(Deserialize)]
struct RuffFix {
    message: Option<String>,
}

#[derive(Deserialize)]
struct RuffDiagnostic {
    code: Option<String>,
    message: String,
    filename: String,
    location: Option<RuffLocation>,
    fix: Option<RuffFix>,
}

#[derive(Deserialize)]
struct MypyDiagnostic {
    file: String,
    line: Option<u32>,
    column: Option<u32>,
    message: String,
    hint: Option<String>,
    code: Option<String>,
    severity: String,
}

#[derive(Deserialize)]
struct PyrightPos {
    line: u32,
    character: u32,
}

#[derive(Deserialize)]
struct PyrightRange {
    start: PyrightPos,
}

#[derive(Deserialize)]
struct PyrightDiagnostic {
    file: String,
    severity: String,
    message: String,
    rule: Option<String>,
    range: Option<PyrightRange>,
}

#[derive(Deserialize)]
struct PyrightReport {
    #[serde(rename = "generalDiagnostics", default)]
    general_diagnostics: Vec<PyrightDiagnostic>,
}

pub(crate) fn parse_non_empty_json<T: DeserializeOwned>(
    raw: &str,
    tool: ExternalToolKind,
) -> Result<Option<T>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|e| ToolParseError::MalformedJson {
            tool,
            detail: e.to_string(),
        })
}

fn ruff_item_to_finding(
    item: RuffDiagnostic,
    root: &Path,
) -> Result<ExternalToolFinding, ToolParseError> {
    let path = normalize_tool_path(&item.filename, root, ExternalToolKind::Ruff)?;
    let (line, column) = item
        .location
        .map_or((1, 1), |loc| (loc.row.max(1), loc.column.max(1)));
    let remediation = item
        .fix
        .and_then(|f| f.message)
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| {
            String::from("Resolve the Ruff violation structurally without `# noqa` comments.")
        });
    Ok(ExternalToolFinding {
        tool: ExternalToolKind::Ruff,
        path,
        line,
        column,
        native_code: item.code.unwrap_or_else(|| String::from("syntax")),
        message: item.message,
        remediation,
    })
}

/// Parses `ruff check --output-format json` output into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid Ruff JSON.
pub fn parse_ruff_json(raw: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(items) = parse_non_empty_json::<Vec<RuffDiagnostic>>(raw, ExternalToolKind::Ruff)?
    else {
        return Ok(Vec::new());
    };
    items
        .into_iter()
        .map(|item| ruff_item_to_finding(item, root))
        .collect()
}

/// Parses `mypy --output json` NDJSON lines into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if any JSON line is malformed.
pub fn parse_mypy_ndjson(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out = Vec::new();
    for line_str in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !line_str.starts_with('{') {
            continue;
        }
        let item: MypyDiagnostic =
            serde_json::from_str(line_str).map_err(|e| ToolParseError::MalformedJson {
                tool: ExternalToolKind::Mypy,
                detail: e.to_string(),
            })?;
        if !item.severity.eq_ignore_ascii_case("error") {
            continue;
        }
        let path = normalize_tool_path(&item.file, root, ExternalToolKind::Mypy)?;
        let remediation = item
            .hint
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map_or_else(
                || String::from("Add strict Python type annotations without `# type: ignore`."),
                |hint| format!("Mypy hint: {hint}"),
            );
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Mypy,
            path,
            line: item.line.unwrap_or(1).max(1),
            column: item.column.unwrap_or(1).max(1),
            native_code: item.code.unwrap_or_else(|| String::from("type-arg")),
            message: item.message,
            remediation,
        });
    }
    Ok(out)
}

fn pyright_item_to_finding(
    diag: PyrightDiagnostic,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    if !diag.severity.eq_ignore_ascii_case("error") {
        return Ok(None);
    }
    let path = normalize_tool_path(&diag.file, root, ExternalToolKind::Pyright)?;
    let (line, column) = diag.range.map_or((1, 1), |r| {
        (
            r.start.line.saturating_add(1),
            r.start.character.saturating_add(1),
        )
    });
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Pyright,
        path,
        line,
        column,
        native_code: diag.rule.unwrap_or_else(|| String::from("type-error")),
        message: diag.message,
        remediation: String::from(
            "Fix the static type mismatch reported by Pyright without `# pyright: ignore`.",
        ),
    }))
}

/// Parses `pyright --outputjson` output (0-indexed lines/cols) into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid Pyright JSON.
pub fn parse_pyright_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(report) = parse_non_empty_json::<PyrightReport>(raw, ExternalToolKind::Pyright)?
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for diag in report.general_diagnostics {
        if let Some(finding) = pyright_item_to_finding(diag, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}
