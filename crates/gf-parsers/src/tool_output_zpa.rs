//! Pure I/O-free Sonar Generic Issue JSON parser for `zpa-cli` (Z PL/SQL Analyzer, `GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback, parse_non_empty_json};

#[derive(Deserialize)]
struct RawZpaTextRange {
    #[serde(default, rename = "startLine")]
    start_line: u32,
    #[serde(default, rename = "startColumn")]
    start_column: u32,
}

#[derive(Deserialize)]
struct RawZpaLocation {
    #[serde(default)]
    message: String,
    #[serde(default, rename = "filePath")]
    file_path: String,
    #[serde(default, rename = "textRange")]
    text_range: Option<RawZpaTextRange>,
}

#[derive(Deserialize)]
struct RawZpaIssue {
    #[serde(default, rename = "ruleId")]
    rule_id: String,
    #[serde(default, rename = "primaryLocation")]
    primary_location: Option<RawZpaLocation>,
}

#[derive(Deserialize)]
struct RawZpaReport {
    #[serde(default)]
    issues: Vec<RawZpaIssue>,
}

/// Parses `zpa-cli` Sonar Generic Issue JSON output into deterministic 1-based [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw_json` is malformed JSON or contains invalid paths.
pub fn parse_zpa_generic_issues_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(report) = parse_non_empty_json::<RawZpaReport>(raw_json, ExternalToolKind::Zpa)?
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(report.issues.len());
    for issue in report.issues {
        let Some(loc) = issue.primary_location else {
            continue;
        };
        let (line, col) = loc
            .text_range
            .map_or((1, 1), |r| (r.start_line.max(1), r.start_column.max(1)));
        let code = if issue.rule_id.trim().is_empty() {
            String::from("zpa")
        } else {
            issue.rule_id
        };
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Zpa,
            path: normalize_tool_path_with_fallback(
                &loc.file_path,
                workspace_root,
                ExternalToolKind::Zpa,
                "package.pkb",
            )?,
            line,
            column: col,
            native_code: code,
            message: loc.message,
            remediation: String::from(
                "Fix the Oracle PL/SQL issue reported by `zpa-cli` (Z PL/SQL Analyzer).",
            ),
        });
    }
    out.sort();
    out.dedup();
    Ok(out)
}
