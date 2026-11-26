//! Pure JSON output parser for `dart analyze --format=json` (`GF082`).

use gf_core::{ExternalToolFinding, ExternalToolKind, RepoPath};
use serde::Deserialize;

use crate::tool_output::ToolParseError;

#[derive(Deserialize)]
struct DartAnalyzeReport {
    #[serde(default)]
    diagnostics: Vec<DartDiag>,
}

#[derive(Deserialize)]
struct DartDiag {
    #[serde(default)]
    code: String,
    #[serde(rename = "problemMessage", default)]
    problem_message: String,
    #[serde(rename = "correctionMessage", default)]
    correction_message: Option<String>,
    location: DartLocation,
}

#[derive(Deserialize)]
struct DartLocation {
    file: String,
    range: DartRange,
}

impl DartLocation {
    const fn start_line_col(&self) -> (u32, u32) {
        let start = &self.range.start;
        (start.line, start.column)
    }
}

#[derive(Deserialize)]
struct DartRange {
    start: DartPos,
}

#[derive(Deserialize)]
struct DartPos {
    #[serde(default)]
    line: u32,
    #[serde(default)]
    column: u32,
}

impl DartDiag {
    fn into_finding(self) -> Option<ExternalToolFinding> {
        let path = RepoPath::new(self.location.file.trim_start_matches("./")).ok()?;
        let (raw_line, raw_col) = self.location.start_line_col();
        let native_code = if self.code.is_empty() {
            String::from("dart_analyze")
        } else {
            self.code
        };
        let remediation = self.correction_message.unwrap_or_else(|| {
            String::from("Fix the `dart analyze` diagnostic at its root cause.")
        });
        Some(ExternalToolFinding {
            tool: ExternalToolKind::DartAnalyze,
            path,
            line: raw_line.max(1),
            column: raw_col.max(1),
            native_code,
            message: self.problem_message,
            remediation,
        })
    }
}

/// Parses `dart analyze --format=json` stdout into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if stdout is not valid JSON.
pub fn parse_dart_analyze_json(stdout: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let report: DartAnalyzeReport =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::DartAnalyze,
            detail: e.to_string(),
        })?;
    Ok(report
        .diagnostics
        .into_iter()
        .filter_map(DartDiag::into_finding)
        .collect())
}
