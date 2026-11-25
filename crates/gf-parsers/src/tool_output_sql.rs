//! Pure I/O-free JSON output parsers (`GF082`) for `squawk`, `sqruff`, and `sqlfluff`.

use std::collections::BTreeMap;
use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback, parse_non_empty_json};

#[derive(Deserialize)]
struct RawSquawkItem {
    #[serde(default)]
    file: String,
    /// `squawk --reporter Json` emits 0-based line numbers!
    #[serde(default)]
    line: u32,
    /// `squawk --reporter Json` emits 0-based column numbers!
    #[serde(default)]
    column: u32,
    #[serde(default)]
    rule_name: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    help: Option<String>,
}

/// Parses `squawk --reporter Json` output into deterministic 1-based [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw_json` is malformed JSON or contains invalid paths.
pub fn parse_squawk_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(items) =
        parse_non_empty_json::<Vec<RawSquawkItem>>(raw_json, ExternalToolKind::Squawk)?
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let code = if item.rule_name.trim().is_empty() {
            String::from("squawk")
        } else {
            item.rule_name
        };
        let remediation = item
            .help
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| {
                String::from("Apply zero-downtime migration patterns recommended by `squawk`.")
            });
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Squawk,
            path: normalize_tool_path_with_fallback(
                &item.file,
                workspace_root,
                ExternalToolKind::Squawk,
                "migrations/0001_init.sql",
            )?,
            line: item.line.saturating_add(1),
            column: item.column.saturating_add(1),
            native_code: code,
            message: item.message,
            remediation,
        });
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[derive(Deserialize)]
struct RawLspPos {
    #[serde(default)]
    line: u32,
    #[serde(default)]
    character: u32,
}

#[derive(Deserialize)]
struct RawLspRange {
    start: RawLspPos,
}

#[derive(Deserialize)]
struct RawSqruffMapEntry {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: String,
    range: Option<RawLspRange>,
}

#[derive(Deserialize)]
struct RawSqlfluffViolation {
    #[serde(default)]
    start_line_no: Option<u32>,
    #[serde(default)]
    line_no: Option<u32>,
    #[serde(default)]
    start_line_pos: Option<u32>,
    #[serde(default)]
    line_pos: Option<u32>,
    #[serde(default)]
    code: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct RawSqlfluffFile {
    #[serde(default)]
    filepath: String,
    #[serde(default)]
    violations: Vec<RawSqlfluffViolation>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SqruffOrFluffPayload {
    FileArray(Vec<RawSqlfluffFile>),
    FileMap(BTreeMap<String, Vec<RawSqruffMapEntry>>),
}

fn collect_sqruff_map_findings(
    map: BTreeMap<String, Vec<RawSqruffMapEntry>>,
    workspace_root: &Path,
    tool: ExternalToolKind,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let default_rem = format!("Fix the SQL lint violation reported by `{tool}`.");
    let mut out = Vec::new();
    for (file_str, entries) in map {
        for entry in entries {
            let (line, col) = entry
                .range
                .map_or((1, 1), |r| (r.start.line.max(1), r.start.character.max(1)));
            out.push(ExternalToolFinding {
                tool,
                path: normalize_tool_path_with_fallback(
                    &file_str,
                    workspace_root,
                    tool,
                    "query.sql",
                )?,
                line,
                column: col,
                native_code: entry.code.unwrap_or_else(|| String::from(tool.as_str())),
                message: entry.message,
                remediation: default_rem.clone(),
            });
        }
    }
    Ok(out)
}

fn collect_sqlfluff_array_findings(
    files: Vec<RawSqlfluffFile>,
    workspace_root: &Path,
    tool: ExternalToolKind,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let default_rem = format!("Fix the SQL lint violation reported by `{tool}`.");
    let mut out = Vec::new();
    for f in files {
        for v in f.violations {
            let line = v.start_line_no.or(v.line_no).unwrap_or(1).max(1);
            let col = v.start_line_pos.or(v.line_pos).unwrap_or(1).max(1);
            let code = if v.code.trim().is_empty() {
                String::from(tool.as_str())
            } else {
                v.code
            };
            out.push(ExternalToolFinding {
                tool,
                path: normalize_tool_path_with_fallback(
                    &f.filepath,
                    workspace_root,
                    tool,
                    "query.sql",
                )?,
                line,
                column: col,
                native_code: code,
                message: v.description,
                remediation: default_rem.clone(),
            });
        }
    }
    Ok(out)
}

fn parse_sqruff_or_fluff(
    raw_json: &str,
    workspace_root: &Path,
    tool: ExternalToolKind,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(payload) = parse_non_empty_json::<SqruffOrFluffPayload>(raw_json, tool)? else {
        return Ok(Vec::new());
    };
    let mut out = match payload {
        SqruffOrFluffPayload::FileMap(map) => {
            collect_sqruff_map_findings(map, workspace_root, tool)?
        }
        SqruffOrFluffPayload::FileArray(files) => {
            collect_sqlfluff_array_findings(files, workspace_root, tool)?
        }
    };
    out.sort();
    out.dedup();
    Ok(out)
}

/// Parses `sqruff lint -f json` output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw_json` is malformed JSON or contains invalid paths.
pub fn parse_sqruff_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    parse_sqruff_or_fluff(raw_json, workspace_root, ExternalToolKind::Sqruff)
}

/// Parses `sqlfluff lint --format json` output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw_json` is malformed JSON or contains invalid paths.
pub fn parse_sqlfluff_json(
    raw_json: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    parse_sqruff_or_fluff(raw_json, workspace_root, ExternalToolKind::Sqlfluff)
}
