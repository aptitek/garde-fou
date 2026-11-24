//! Pure I/O-free JSON and NDJSON output parsers for `ruff`, `mypy`, and `pyright` (`GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolId, RepoPath};
use serde::Deserialize;

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

fn strip_drive_letter(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        s.get(2..).unwrap_or(s)
    } else {
        s
    }
}

fn normalize_tool_path(raw: &str, root: &Path) -> Result<RepoPath, String> {
    let trimmed = raw.trim();
    let slash_path = trimmed
        .strip_prefix("file://")
        .unwrap_or(trimmed)
        .replace('\\', "/");
    let slash_path = if slash_path.starts_with('/')
        && slash_path.as_bytes().get(2) == Some(&b':')
        && slash_path.get(1..).is_some()
    {
        slash_path.get(1..).unwrap_or(&slash_path)
    } else {
        slash_path.as_str()
    };

    let root_norm = root
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    let stripped = if !root_norm.is_empty()
        && slash_path
            .get(..root_norm.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(&root_norm))
    {
        slash_path.get(root_norm.len()..).unwrap_or(slash_path)
    } else {
        let no_drive = strip_drive_letter(slash_path);
        let root_no_drive = strip_drive_letter(&root_norm);
        if !root_no_drive.is_empty()
            && no_drive
                .get(..root_no_drive.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(root_no_drive))
        {
            no_drive.get(root_no_drive.len()..).unwrap_or(no_drive)
        } else {
            no_drive
        }
    };

    let rel = stripped.trim_start_matches("./").trim_start_matches('/');
    let target = if rel.is_empty() {
        "pyproject.toml"
    } else {
        rel
    };
    RepoPath::new(target)
        .or_else(|_| RepoPath::new("pyproject.toml"))
        .map_err(|e| e.to_string())
}

/// Parses `ruff check --output-format json` output into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns an error string if `raw` is non-empty and not valid Ruff JSON.
pub fn parse_ruff_json(raw: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<RuffDiagnostic> =
        serde_json::from_str(trimmed).map_err(|e| format!("malformed ruff JSON: {e}"))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let path = normalize_tool_path(&item.filename, root)?;
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
        out.push(ExternalToolFinding {
            tool: ExternalToolId::Ruff,
            path,
            line,
            column,
            native_code: item.code.unwrap_or_else(|| String::from("syntax")),
            message: item.message,
            remediation,
        });
    }
    Ok(out)
}

/// Parses `mypy --output json` NDJSON lines into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns an error string if any JSON line is malformed.
pub fn parse_mypy_ndjson(raw: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
    let mut out = Vec::new();
    for line_str in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !line_str.starts_with('{') {
            continue;
        }
        let item: MypyDiagnostic =
            serde_json::from_str(line_str).map_err(|e| format!("malformed mypy NDJSON: {e}"))?;
        if !item.severity.eq_ignore_ascii_case("error") {
            continue;
        }
        let path = normalize_tool_path(&item.file, root)?;
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
            tool: ExternalToolId::Mypy,
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

/// Parses `pyright --outputjson` output (0-indexed lines/cols) into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns an error string if `raw` is non-empty and not valid Pyright JSON.
pub fn parse_pyright_json(raw: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let report: PyrightReport =
        serde_json::from_str(trimmed).map_err(|e| format!("malformed pyright JSON: {e}"))?;
    let mut out = Vec::new();
    for diag in report.general_diagnostics {
        if !diag.severity.eq_ignore_ascii_case("error") {
            continue;
        }
        let path = normalize_tool_path(&diag.file, root)?;
        let (line, column) = diag.range.map_or((1, 1), |r| {
            (
                r.start.line.saturating_add(1),
                r.start.character.saturating_add(1),
            )
        });
        out.push(ExternalToolFinding {
            tool: ExternalToolId::Pyright,
            path,
            line,
            column,
            native_code: diag.rule.unwrap_or_else(|| String::from("type-error")),
            message: diag.message,
            remediation: String::from(
                "Fix the static type mismatch reported by Pyright without `# pyright: ignore`.",
            ),
        });
    }
    Ok(out)
}
