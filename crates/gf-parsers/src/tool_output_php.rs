//! Pure JSON output parsers for `PHPStan` (`--error-format=json`) and `Psalm` (`--output-format=json`) (`GF082`).

use std::collections::BTreeMap;

use gf_core::{ExternalToolFinding, ExternalToolKind, RepoPath};
use serde::Deserialize;

use crate::tool_output::ToolParseError;

#[derive(Deserialize)]
struct PhpstanReport {
    #[serde(default)]
    files: BTreeMap<String, PhpstanFile>,
}

#[derive(Deserialize)]
struct PhpstanFile {
    #[serde(default)]
    messages: Vec<PhpstanMessage>,
}

#[derive(Deserialize)]
struct PhpstanMessage {
    message: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    tip: Option<String>,
}

/// Parses `phpstan analyse --error-format=json` stdout into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if stdout is not valid JSON.
pub fn parse_phpstan_json(stdout: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let report: PhpstanReport =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Phpstan,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for (file, entry) in report.files {
        let Ok(path) = RepoPath::new(file.trim_start_matches("./")) else {
            continue;
        };
        for msg in entry.messages {
            out.push(ExternalToolFinding {
                tool: ExternalToolKind::Phpstan,
                path: path.clone(),
                line: msg.line.unwrap_or(1).max(1),
                column: 1,
                native_code: msg
                    .identifier
                    .unwrap_or_else(|| String::from("phpstan.error")),
                message: msg.message,
                remediation: msg.tip.unwrap_or_else(|| {
                    String::from("Fix the PHPStan type/static-analysis error at its root cause.")
                }),
            });
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct PsalmItem {
    file_name: String,
    #[serde(default)]
    line_from: u32,
    #[serde(default)]
    column_from: u32,
    #[serde(rename = "type", default)]
    issue_type: String,
    message: String,
}

/// Parses `psalm --output-format=json` stdout into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if stdout is not valid JSON.
pub fn parse_psalm_json(stdout: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<PsalmItem> =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Psalm,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for item in items {
        let Ok(path) = RepoPath::new(item.file_name.trim_start_matches("./")) else {
            continue;
        };
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Psalm,
            path,
            line: item.line_from.max(1),
            column: item.column_from.max(1),
            native_code: if item.issue_type.is_empty() {
                String::from("PsalmError")
            } else {
                item.issue_type
            },
            message: item.message,
            remediation: String::from("Fix the Psalm static-analysis finding at its root cause."),
        });
    }
    Ok(out)
}
