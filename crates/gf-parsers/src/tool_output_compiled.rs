//! Pure I/O-free Tier 3 tool-output parsers for Go (`golangci-lint`, `go vet`),
//! Kotlin (`ktlint`), and C# (`dotnet build` / `MSBuild`) (`GF082`).

use std::collections::BTreeMap;

use gf_core::{ExternalToolFinding, ExternalToolKind, RepoPath};
use serde::Deserialize;

use crate::tool_output::ToolParseError;

fn normalize_tool_path(raw: &str) -> Option<RepoPath> {
    let slash = raw.trim().replace('\\', "/");
    let stripped = slash
        .strip_prefix("./")
        .unwrap_or(&slash)
        .trim_start_matches('/');
    let rel = if stripped.len() >= 3
        && stripped.as_bytes()[1] == b':'
        && stripped.as_bytes()[2] == b'/'
    {
        &stripped[3..]
    } else {
        stripped
    };
    RepoPath::new(rel).ok()
}

#[derive(Deserialize)]
struct GolangciReport {
    #[serde(rename = "Issues", default)]
    issues: Vec<GolangciIssue>,
}

#[derive(Deserialize)]
struct GolangciIssue {
    #[serde(rename = "FromLinter", default)]
    from_linter: String,
    #[serde(rename = "Text", default)]
    text: String,
    #[serde(rename = "Pos")]
    pos: GolangciPos,
}

#[derive(Deserialize)]
struct GolangciPos {
    #[serde(rename = "Filename", default)]
    filename: String,
    #[serde(rename = "Line", default)]
    line: u32,
    #[serde(rename = "Column", default)]
    column: u32,
}

/// Parses `golangci-lint` v1 & v2 JSON output (`{ "Issues": [...] }`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is not valid JSON matching the `golangci-lint` schema.
pub fn parse_golangci_lint_json(raw: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let report: GolangciReport =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::GolangciLint,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for issue in report.issues {
        if let Some(path) = normalize_tool_path(&issue.pos.filename) {
            out.push(ExternalToolFinding {
                tool: ExternalToolKind::GolangciLint,
                path,
                line: issue.pos.line.max(1),
                column: issue.pos.column.max(1),
                native_code: if issue.from_linter.is_empty() {
                    String::from("golangci-lint")
                } else {
                    issue.from_linter
                },
                message: issue.text,
                remediation: String::from("Resolve the reported `golangci-lint` issue at its root cause without adding bare `//nolint`."),
            });
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct GoVetDiag {
    #[serde(default)]
    posn: String,
    #[serde(default)]
    message: String,
}

fn build_go_vet_finding(analyzer: &str, d: GoVetDiag) -> Option<ExternalToolFinding> {
    let mut parts = d.posn.rsplitn(3, ':');
    let col = parts.next()?.parse::<u32>().ok().unwrap_or(1);
    let line = parts.next()?.parse::<u32>().ok()?;
    let path = normalize_tool_path(parts.next()?)?;
    Some(ExternalToolFinding {
        tool: ExternalToolKind::GoVet,
        path,
        line: line.max(1),
        column: col.max(1),
        native_code: String::from(analyzer),
        message: d.message,
        remediation: String::from("Fix the `go vet` static analysis issue reported here."),
    })
}

/// Parses concatenated JSON objects emitted by `go vet -json ./...`.
///
/// # Errors
///
/// Returns [`ToolParseError`] if any JSON object in the stream is malformed.
pub fn parse_go_vet_json(raw: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    type VetPkg = BTreeMap<String, BTreeMap<String, Vec<GoVetDiag>>>;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for item in serde_json::Deserializer::from_str(trimmed).into_iter::<VetPkg>() {
        let pkg_map = item.map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::GoVet,
            detail: e.to_string(),
        })?;
        for (analyzer, diags) in pkg_map.into_values().flatten() {
            out.extend(
                diags
                    .into_iter()
                    .filter_map(|d| build_go_vet_finding(&analyzer, d)),
            );
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct KtlintFileReport {
    #[serde(default)]
    file: String,
    #[serde(default)]
    errors: Vec<KtlintError>,
}

#[derive(Deserialize)]
struct KtlintError {
    #[serde(default)]
    line: u32,
    #[serde(default)]
    column: u32,
    #[serde(default)]
    message: String,
    #[serde(default)]
    rule: String,
}

/// Parses `ktlint --reporter=json` output (`[{ "file", "errors": [...] }]`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is not valid `ktlint` JSON.
pub fn parse_ktlint_json(raw: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let files: Vec<KtlintFileReport> =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::Ktlint,
            detail: e.to_string(),
        })?;
    let mut out = Vec::new();
    for f in files {
        let Some(path) = normalize_tool_path(&f.file) else {
            continue;
        };
        for err in f.errors {
            out.push(ExternalToolFinding {
                tool: ExternalToolKind::Ktlint,
                path: path.clone(),
                line: err.line.max(1),
                column: err.column.max(1),
                native_code: if err.rule.is_empty() {
                    String::from("ktlint")
                } else {
                    err.rule
                },
                message: err.message,
                remediation: String::from("Format or refactor the Kotlin file to satisfy `ktlint` without suppression comments."),
            });
        }
    }
    Ok(out)
}

fn parse_msbuild_line(line: &str) -> Option<ExternalToolFinding> {
    let (file_loc, rest) = line.split_once("): ")?;
    let (file_raw, coords) = file_loc.rsplit_once('(')?;
    let mut coord_parts = coords.split(',');
    let line_no = coord_parts.next()?.trim().parse::<u32>().ok()?;
    let col_no = coord_parts
        .next()
        .and_then(|c| c.trim().parse::<u32>().ok())
        .unwrap_or(1);
    let after_sev = rest
        .strip_prefix("error ")
        .or_else(|| rest.strip_prefix("warning "))?;
    let (code, msg_tail) = after_sev.split_once(':')?;
    let msg = msg_tail
        .rsplit_once(" [")
        .map_or(msg_tail, |(m, _)| m)
        .trim();
    let path = normalize_tool_path(file_raw)?;
    Some(ExternalToolFinding {
        tool: ExternalToolKind::DotnetBuild,
        path,
        line: line_no.max(1),
        column: col_no.max(1),
        native_code: String::from(code.trim()),
        message: String::from(msg),
        remediation: String::from(
            "Fix the C# Roslyn / MSBuild diagnostic at its root cause instead of adding `#pragma warning disable`.",
        ),
    })
}

/// Parses canonical `dotnet build --no-restore -nologo -v:q` (`MSBuild`) diagnostic lines
/// (`path/Service.cs(42,13): error CS8602: Message [Project.csproj]`), supporting Unix and Windows paths.
///
/// # Errors
///
/// Never fails on non-diagnostic build lines (skips noise deterministically).
pub fn parse_msbuild_diagnostics(raw: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    Ok(raw.lines().filter_map(parse_msbuild_line).collect())
}
