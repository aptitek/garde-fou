//! Pure I/O-free output parsers (`GF082`) for `yamllint -f parsable` and `dclint -f json`
//! (with `actionlint` and `zizmor` re-exported from [`crate::tool_output_gha`]).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback};
pub use crate::tool_output_gha::{parse_actionlint_json, parse_zizmor_sarif};

fn extract_trailing_paren_rule<'a>(msg_and_rule: &'a str, fallback: &'a str) -> (&'a str, &'a str) {
    let trimmed = msg_and_rule.trim();
    if let Some(without_close) = trimmed.strip_suffix(')')
        && let Some((msg, rule)) = without_close.rsplit_once(" (")
        && !rule.trim().is_empty()
    {
        return (msg.trim(), rule.trim());
    }
    (trimmed, fallback)
}

fn parse_yamllint_line(
    line: &str,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    let Some((loc_part, after_bracket)) = line.split_once(": [") else {
        return Ok(None);
    };
    let Some((level, rest_msg)) = after_bracket.split_once(']') else {
        return Ok(None);
    };
    if !matches!(
        level.trim().to_ascii_lowercase().as_str(),
        "error" | "warning"
    ) {
        return Ok(None);
    }
    let Some((before_col, col_str)) = loc_part.rsplit_once(':') else {
        return Ok(None);
    };
    let Some((raw_path, line_str)) = before_col.rsplit_once(':') else {
        return Ok(None);
    };
    let (Ok(line_no), Ok(col_no)) = (
        line_str.trim().parse::<u32>(),
        col_str.trim().parse::<u32>(),
    ) else {
        return Ok(None);
    };
    let (msg, rule_code) = extract_trailing_paren_rule(rest_msg, "yamllint");
    let path = normalize_tool_path_with_fallback(
        raw_path,
        root,
        ExternalToolKind::Yamllint,
        "config.yaml",
    )?;
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Yamllint,
        path,
        line: line_no.max(1),
        column: col_no.max(1),
        native_code: String::from(rule_code),
        message: String::from(msg),
        remediation: String::from(
            "Fix the YAML 1.2 formatting or structure issue reported by `yamllint`.",
        ),
    }))
}

/// Parses `yamllint -f parsable` lines (`<path>:<line>:<col>: [<level>] <msg> (<rule>)`),
/// preserving Windows `C:\...` drive-letter colons via right-to-left `:` splitting.
///
/// # Errors
///
/// Returns [`ToolParseError::InvalidPath`] only if a fallback path cannot be constructed.
pub fn parse_yamllint_parsable(
    stdout: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|line| parse_yamllint_line(line, root).transpose())
        .collect()
}

#[derive(Deserialize)]
struct DclintMessage {
    rule: Option<String>,
    message: String,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
}

#[derive(Deserialize)]
struct DclintFileReport {
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(default)]
    messages: Vec<DclintMessage>,
}

/// Parses `dclint -f json` output into normalized [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if `stdout` is malformed JSON.
pub fn parse_dclint_json(
    stdout: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let files: Vec<DclintFileReport> =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Dclint,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for file_rep in files {
        let path = normalize_tool_path_with_fallback(
            &file_rep.file_path,
            root,
            ExternalToolKind::Dclint,
            "compose.yaml",
        )?;
        for m in file_rep.messages {
            let keep = m.msg_type.as_deref().is_none_or(|t| {
                matches!(
                    t.to_ascii_lowercase().as_str(),
                    "error" | "warning" | "warn"
                )
            });
            if !keep {
                continue;
            }
            out.push(ExternalToolFinding {
                tool: ExternalToolKind::Dclint,
                path: path.clone(),
                line: m.line.unwrap_or(1).max(1),
                column: m.column.unwrap_or(1).max(1),
                native_code: m.rule.unwrap_or_else(|| String::from("dclint")),
                message: m.message,
                remediation: String::from(
                    "Remediate the Docker Compose specification issue reported by `dclint`.",
                ),
            });
        }
    }
    Ok(out)
}
