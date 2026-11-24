//! Pure I/O-free JSON output parsers for `oxlint` and `biome` (`GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

pub use crate::tool_output::parse_tsc_output;
use crate::tool_output::{
    ToolParseError, byte_offset_to_line_col, normalize_tool_path_with_fallback,
};

#[derive(Deserialize)]
struct OxlintSpan {
    line: Option<u32>,
    column: Option<u32>,
}

#[derive(Deserialize)]
struct OxlintLabel {
    span: Option<OxlintSpan>,
}

#[derive(Deserialize)]
struct OxlintItem {
    message: String,
    code: Option<String>,
    severity: Option<String>,
    help: Option<String>,
    filename: Option<String>,
    #[serde(default)]
    labels: Vec<OxlintLabel>,
    start_line: Option<u32>,
    line: Option<u32>,
    start_column: Option<u32>,
    column: Option<u32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OxlintEnvelope {
    Wrapped { diagnostics: Vec<OxlintItem> },
    Bare(Vec<OxlintItem>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BiomePathResource {
    Tagged { file: String },
    Plain(String),
}

#[derive(Deserialize)]
struct BiomeLineCol {
    line: u32,
    column: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BiomeLocation {
    path: Option<BiomePathResource>,
    span: Option<[usize; 2]>,
    source_code: Option<String>,
    start: Option<BiomeLineCol>,
}

#[derive(Deserialize)]
struct BiomeMarkupNode {
    content: Option<String>,
}

#[derive(Deserialize)]
struct BiomeItem {
    category: Option<String>,
    severity: Option<String>,
    description: Option<String>,
    #[serde(default)]
    message: Vec<BiomeMarkupNode>,
    location: Option<BiomeLocation>,
}

#[derive(Deserialize)]
struct BiomeReport {
    #[serde(default)]
    diagnostics: Vec<BiomeItem>,
}

fn oxlint_item_to_finding(
    item: OxlintItem,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    if item
        .severity
        .as_deref()
        .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "off" | "allow" | "advice"))
    {
        return Ok(None);
    }
    let raw_file = item.filename.as_deref().unwrap_or("package.json");
    let path = normalize_tool_path_with_fallback(
        raw_file,
        root,
        ExternalToolKind::Oxlint,
        "package.json",
    )?;
    let lbl_span = item.labels.first().and_then(|l| l.span.as_ref());
    let line = lbl_span
        .and_then(|s| s.line)
        .or(item.start_line)
        .or(item.line)
        .unwrap_or(1)
        .max(1);
    let column = lbl_span
        .and_then(|s| s.column)
        .or(item.start_column)
        .or(item.column)
        .unwrap_or(1)
        .max(1);
    let remediation = item
        .help
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map_or_else(
            || String::from("Resolve the `oxlint` violation structurally without `eslint-disable` comments."),
            |help| format!("Oxlint help: {help}"),
        );
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Oxlint,
        path,
        line,
        column,
        native_code: item.code.unwrap_or_else(|| String::from("oxlint")),
        message: item.message,
        remediation,
    }))
}

/// Parses `oxlint --format json` output (`{ diagnostics: [...] }` or `[...]`) into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid Oxlint JSON.
pub fn parse_oxlint_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let env: OxlintEnvelope =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Oxlint,
            detail: e.to_string(),
        })?;
    let items = match env {
        OxlintEnvelope::Wrapped { diagnostics } | OxlintEnvelope::Bare(diagnostics) => diagnostics,
    };
    let mut out = Vec::new();
    for item in items {
        if let Some(finding) = oxlint_item_to_finding(item, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}

fn biome_item_to_finding(
    diag: BiomeItem,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    if !diag
        .severity
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("error"))
    {
        return Ok(None);
    }
    let raw_file = diag
        .location
        .as_ref()
        .and_then(|l| l.path.as_ref())
        .map_or("biome.json", |p| match p {
            BiomePathResource::Tagged { file } | BiomePathResource::Plain(file) => file.as_str(),
        });
    let path =
        normalize_tool_path_with_fallback(raw_file, root, ExternalToolKind::Biome, "biome.json")?;
    let (line, column) = diag.location.as_ref().map_or((1, 1), |loc| {
        loc.start.as_ref().map_or_else(
            || match (loc.span, loc.source_code.as_deref()) {
                (Some([start, _]), Some(src)) => byte_offset_to_line_col(src, start),
                _ => (1, 1),
            },
            |st| (st.line.max(1), st.column.max(1)),
        )
    });
    let msg = diag
        .description
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| {
            diag.message
                .iter()
                .filter_map(|m| m.content.as_deref())
                .collect::<String>()
        });
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Biome,
        path,
        line,
        column,
        native_code: diag.category.unwrap_or_else(|| String::from("lint")),
        message: msg,
        remediation: String::from(
            "Fix the Biome error structurally without `biome-ignore` comments.",
        ),
    }))
}

/// Parses `biome check --reporter=json` output into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid Biome JSON.
pub fn parse_biome_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let report: BiomeReport =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Biome,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for diag in report.diagnostics {
        if let Some(finding) = biome_item_to_finding(diag, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}
