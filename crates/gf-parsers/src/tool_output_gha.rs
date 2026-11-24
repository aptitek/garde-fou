//! Pure I/O-free output parsers (`GF082`) for `actionlint -format '{{json .}}'`
//! and `zizmor --format sarif`.

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback, parse_non_empty_json};

#[derive(Deserialize)]
struct ActionlintEntry {
    message: String,
    filepath: String,
    line: Option<u32>,
    column: Option<u32>,
    kind: Option<String>,
}

/// Parses `actionlint -format '{{json .}}'` JSON array output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `stdout` is malformed JSON.
pub fn parse_actionlint_json(
    stdout: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(entries) =
        parse_non_empty_json::<Vec<ActionlintEntry>>(stdout, ExternalToolKind::Actionlint)?
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let path = normalize_tool_path_with_fallback(
            &e.filepath,
            root,
            ExternalToolKind::Actionlint,
            ".github/workflows/ci.yml",
        )?;
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Actionlint,
            path,
            line: e.line.unwrap_or(1).max(1),
            column: e.column.unwrap_or(1).max(1),
            native_code: e.kind.unwrap_or_else(|| String::from("actionlint")),
            message: e.message,
            remediation: String::from(
                "Fix the GitHub Actions workflow type, expression, or runner issue reported by `actionlint`.",
            ),
        });
    }
    Ok(out)
}

#[derive(Deserialize)]
struct SarifText {
    text: String,
}

#[derive(Deserialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: Option<u32>,
    #[serde(rename = "startColumn")]
    start_column: Option<u32>,
}

#[derive(Deserialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Deserialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: Option<SarifRegion>,
}

#[derive(Deserialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: Option<SarifPhysicalLocation>,
}

#[derive(Deserialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    level: Option<String>,
    message: SarifText,
    #[serde(default)]
    locations: Vec<SarifLocation>,
}

#[derive(Deserialize)]
struct SarifRun {
    #[serde(default)]
    results: Vec<SarifResult>,
}

#[derive(Deserialize)]
struct SarifLog {
    #[serde(default)]
    runs: Vec<SarifRun>,
}

fn sarif_result_to_finding(
    res: SarifResult,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    if res
        .level
        .as_deref()
        .is_some_and(|lvl| matches!(lvl, "none" | "note"))
    {
        return Ok(None);
    }
    let phys = res
        .locations
        .iter()
        .find_map(|l| l.physical_location.as_ref());
    let raw_uri = phys.map_or(".github/workflows/ci.yml", |p| {
        p.artifact_location.uri.as_str()
    });
    let (line, col) = phys.and_then(|p| p.region.as_ref()).map_or((1, 1), |r| {
        (
            r.start_line.unwrap_or(1).max(1),
            r.start_column.unwrap_or(1).max(1),
        )
    });
    let path = normalize_tool_path_with_fallback(
        raw_uri,
        root,
        ExternalToolKind::Zizmor,
        ".github/workflows/ci.yml",
    )?;
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Zizmor,
        path,
        line,
        column: col,
        native_code: res.rule_id.unwrap_or_else(|| String::from("zizmor")),
        message: res.message.text,
        remediation: String::from(
            "Remediate the GitHub Actions workflow vulnerability reported by `zizmor`.",
        ),
    }))
}

/// Parses `zizmor --format sarif` SARIF 2.1.0 output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `stdout` is malformed SARIF JSON.
pub fn parse_zizmor_sarif(
    stdout: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let Some(sarif) = parse_non_empty_json::<SarifLog>(stdout, ExternalToolKind::Zizmor)? else {
        return Ok(Vec::new());
    };
    sarif
        .runs
        .into_iter()
        .flat_map(|run| run.results)
        .filter_map(|res| sarif_result_to_finding(res, root).transpose())
        .collect()
}
