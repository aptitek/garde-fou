//! Pure I/O-free Tier 3 output parsers (`GF082`) for `stylelint` and `rumdl` / `markdownlint`
//! (with `taplo` and `xmllint` re-exported from [`crate::tool_output_taplo_xml`]).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback, parse_non_empty_json};
pub use crate::tool_output_taplo_xml::{parse_taplo_output, parse_xmllint_output};

#[derive(Deserialize)]
struct StylelintFileEntry {
    source: Option<String>,
    #[serde(default)]
    warnings: Vec<StylelintWarning>,
}

#[derive(Deserialize)]
struct StylelintWarning {
    line: Option<u32>,
    column: Option<u32>,
    rule: Option<String>,
    text: String,
}

fn stylelint_entry_to_findings(
    entry: StylelintFileEntry,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let path = normalize_tool_path_with_fallback(
        entry.source.as_deref().unwrap_or("styles.css"),
        workspace_root,
        ExternalToolKind::Stylelint,
        "styles.css",
    )?;
    Ok(entry
        .warnings
        .into_iter()
        .map(|w| ExternalToolFinding {
            tool: ExternalToolKind::Stylelint,
            path: path.clone(),
            line: w.line.unwrap_or(1).max(1),
            column: w.column.unwrap_or(1).max(1),
            native_code: w.rule.unwrap_or_else(|| String::from("stylelint")),
            message: w.text,
            remediation: String::from(
                "Fix the CSS syntax, specificity, or token violation reported by `stylelint`.",
            ),
        })
        .collect())
}

/// Parses `stylelint --formatter json` output into deterministic [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError::MalformedJson`] if non-empty input is not valid JSON.
pub fn parse_stylelint_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(files) =
        parse_non_empty_json::<Vec<StylelintFileEntry>>(raw_json, ExternalToolKind::Stylelint)?
    else {
        return Ok(Vec::new());
    };
    let batches: Vec<Vec<ExternalToolFinding>> = files
        .into_iter()
        .map(|entry| stylelint_entry_to_findings(entry, workspace_root))
        .collect::<Result<_, _>>()?;
    let mut out: Vec<ExternalToolFinding> = batches.into_iter().flatten().collect();
    out.sort();
    out.dedup();
    Ok(out)
}

#[derive(Deserialize)]
struct UnifiedMdDiag {
    file: Option<String>,
    #[serde(rename = "fileName")]
    file_name: Option<String>,
    line: Option<u32>,
    #[serde(rename = "lineNumber")]
    line_number: Option<u32>,
    column: Option<u32>,
    #[serde(rename = "errorRange")]
    error_range: Option<[u32; 2]>,
    rule: Option<String>,
    #[serde(default, rename = "ruleNames")]
    rule_names: Vec<String>,
    message: Option<String>,
    #[serde(rename = "ruleDescription")]
    rule_description: Option<String>,
    #[serde(rename = "errorDetail")]
    error_detail: Option<String>,
}

fn convert_md_diag(
    item: UnifiedMdDiag,
    workspace_root: &Path,
) -> Result<ExternalToolFinding, ToolParseError> {
    let tool = if item.file.is_some() || item.rule.is_some() {
        ExternalToolKind::Rumdl
    } else {
        ExternalToolKind::Markdownlint
    };
    let raw_file = item
        .file
        .as_deref()
        .or(item.file_name.as_deref())
        .unwrap_or("README.md");
    let line = item.line.or(item.line_number).unwrap_or(1).max(1);
    let column = item
        .column
        .or_else(|| item.error_range.map(|[c, _]| c))
        .unwrap_or(1)
        .max(1);
    let native_code = item
        .rule
        .or_else(|| item.rule_names.into_iter().next())
        .unwrap_or_else(|| String::from("MD000"));
    let message = match (item.message, item.rule_description, item.error_detail) {
        (Some(m), _, _) => m,
        (None, Some(desc), Some(det)) => format!("{desc}: {det}"),
        (None, Some(desc), None) => desc,
        (None, None, Some(det)) => det,
        (None, None, None) => String::from("Markdown lint violation"),
    };
    Ok(ExternalToolFinding {
        tool,
        path: normalize_tool_path_with_fallback(raw_file, workspace_root, tool, "README.md")?,
        line,
        column,
        native_code,
        message,
        remediation: String::from(
            "Fix the CommonMark/GFM structure or link issue at its root cause.",
        ),
    })
}

/// Parses `rumdl check --output-format json` or `markdownlint --json` output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError::MalformedJson`] if non-empty input is not a JSON array.
pub fn parse_rumdl_or_markdownlint_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<UnifiedMdDiag> =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Rumdl,
            detail: e.to_string(),
        })?;
    let mut out: Vec<ExternalToolFinding> = items
        .into_iter()
        .map(|it| convert_md_diag(it, workspace_root))
        .collect::<Result<_, _>>()?;
    out.sort();
    out.dedup();
    Ok(out)
}
