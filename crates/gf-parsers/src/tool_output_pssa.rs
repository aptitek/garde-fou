//! Pure I/O-free JSON output parser for `PSScriptAnalyzer` (`GF082`), handling
//! PowerShell 5.1's `ConvertTo-Json` empty/`null`, single-object `{...}`, and array `[{...}]` shapes.

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback};

#[derive(Deserialize)]
#[serde(untagged)]
enum PssaSeverity {
    Text(String),
    Code(u8),
}

impl PssaSeverity {
    fn is_error_or_warning(&self) -> bool {
        match self {
            Self::Text(s) => matches!(
                s.trim().to_ascii_lowercase().as_str(),
                "error" | "warning" | "parseerror"
            ),
            Self::Code(n) => *n >= 1,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PssaRecord {
    rule_name: Option<String>,
    script_path: Option<String>,
    script_name: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    message: String,
    severity: Option<PssaSeverity>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PssaEnvelope {
    Many(Vec<PssaRecord>),
    Single(Box<PssaRecord>),
}

fn pssa_record_to_finding(
    rec: PssaRecord,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    if rec
        .severity
        .as_ref()
        .is_some_and(|sev| !sev.is_error_or_warning())
    {
        return Ok(None);
    }
    let raw_file = rec
        .script_path
        .as_deref()
        .or(rec.script_name.as_deref())
        .unwrap_or("script.ps1");
    let path = normalize_tool_path_with_fallback(
        raw_file,
        root,
        ExternalToolKind::PsScriptAnalyzer,
        "script.ps1",
    )?;
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::PsScriptAnalyzer,
        path,
        line: rec.line.unwrap_or(1).max(1),
        column: rec.column.unwrap_or(1).max(1),
        native_code: rec
            .rule_name
            .unwrap_or_else(|| String::from("PSScriptAnalyzer")),
        message: rec.message,
        remediation: String::from(
            "Fix the `PSScriptAnalyzer` finding structurally without `[SuppressMessageAttribute]`.",
        ),
    }))
}

/// Parses `Invoke-ScriptAnalyzer | ConvertTo-Json` output (`""`/`"null"`, `{...}`, or `[{...}]`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty/non-`null` and not valid `PSScriptAnalyzer` JSON.
pub fn parse_psscriptanalyzer_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    let env: PssaEnvelope =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::PsScriptAnalyzer,
            detail: e.to_string(),
        })?;
    let records = match env {
        PssaEnvelope::Many(list) => list,
        PssaEnvelope::Single(one) => vec![*one],
    };
    let mut out = Vec::new();
    for rec in records {
        if let Some(finding) = pssa_record_to_finding(rec, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}
