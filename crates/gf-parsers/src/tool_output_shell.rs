//! Pure I/O-free JSON output parsers for `shellcheck`, `hadolint`, and `PSScriptAnalyzer` (`GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback};
pub use crate::tool_output_pssa::parse_psscriptanalyzer_json;

#[derive(Deserialize)]
#[serde(untagged)]
enum ShellcheckCode {
    Number(u32),
    Text(String),
}

impl ShellcheckCode {
    fn into_code_string(self) -> String {
        match self {
            Self::Number(n) => format!("SC{n}"),
            Self::Text(s) => {
                let t = s.trim();
                if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
                    format!("SC{t}")
                } else {
                    String::from(t)
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct RawLineColComment {
    file: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    level: Option<String>,
    code: Option<ShellcheckCode>,
    message: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ShellcheckEnvelope {
    Json1 { comments: Vec<RawLineColComment> },
    Flat(Vec<RawLineColComment>),
}

struct LineColToolSpec<'a> {
    tool: ExternalToolKind,
    fallback_file: &'a str,
    fallback_code: &'a str,
    remediation: &'a str,
}

fn is_error_or_warning_level(level: Option<&str>) -> bool {
    level.is_none_or(|lvl| {
        matches!(
            lvl.trim().to_ascii_lowercase().as_str(),
            "error" | "warning"
        )
    })
}

fn normalize_line_col_comments(
    items: Vec<RawLineColComment>,
    root: &Path,
    spec: &LineColToolSpec<'_>,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out = Vec::new();
    for item in items {
        if !is_error_or_warning_level(item.level.as_deref()) {
            continue;
        }
        let raw_file = item.file.as_deref().unwrap_or(spec.fallback_file);
        let path =
            normalize_tool_path_with_fallback(raw_file, root, spec.tool, spec.fallback_file)?;
        let code = item.code.map_or_else(
            || String::from(spec.fallback_code),
            ShellcheckCode::into_code_string,
        );
        out.push(ExternalToolFinding {
            tool: spec.tool,
            path,
            line: item.line.unwrap_or(1).max(1),
            column: item.column.unwrap_or(1).max(1),
            native_code: code,
            message: item.message,
            remediation: String::from(spec.remediation),
        });
    }
    Ok(out)
}

/// Parses `shellcheck --format=json1` (`{ "comments": [...] }` or `[...]`) into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid `ShellCheck` JSON.
pub fn parse_shellcheck_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let env: ShellcheckEnvelope =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Shellcheck,
            detail: e.to_string(),
        })?;
    let comments = match env {
        ShellcheckEnvelope::Json1 { comments } | ShellcheckEnvelope::Flat(comments) => comments,
    };
    let spec = LineColToolSpec {
        tool: ExternalToolKind::Shellcheck,
        fallback_file: "script.sh",
        fallback_code: "SC0000",
        remediation: "Fix the `ShellCheck` violation structurally instead of adding `# shellcheck disable`.",
    };
    normalize_line_col_comments(comments, root, &spec)
}

/// Parses `hadolint --format json` (`[{ file, line, column, code, message, level }]`) into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid `Hadolint` JSON.
pub fn parse_hadolint_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<RawLineColComment> =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Hadolint,
            detail: e.to_string(),
        })?;
    let spec = LineColToolSpec {
        tool: ExternalToolKind::Hadolint,
        fallback_file: "Dockerfile",
        fallback_code: "DL0000",
        remediation: "Resolve the `Hadolint` container build violation without `# hadolint ignore`.",
    };
    normalize_line_col_comments(items, root, &spec)
}
