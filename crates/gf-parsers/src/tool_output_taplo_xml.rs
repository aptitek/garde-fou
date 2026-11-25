//! Pure I/O-free output parsers (`GF082`) for `taplo check` and `xmllint --noout`
//! (Windows `C:\...` drive-letter safe).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback};

fn split_win_safe_colon(raw: &str) -> Option<(&str, &str)> {
    let start_idx = match raw.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic() => 3,
        _ => 0,
    };
    let rel = raw.get(start_idx..)?.find(':')?;
    let colon = start_idx + rel;
    Some((&raw[..colon], &raw[colon + 1..]))
}

fn parse_taplo_header(trimmed: &str) -> Option<(String, String)> {
    let after_err = trimmed
        .strip_prefix("error")
        .or_else(|| trimmed.strip_prefix("warning"))?;
    let (code_part, msg) = after_err.split_once(':')?;
    let code = code_part
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .filter(|s| !s.is_empty())
        .unwrap_or("taplo");
    Some((String::from(code), String::from(msg.trim())))
}

fn build_taplo_finding(
    loc_str: &str,
    header: (String, String),
    workspace_root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    let Some((file_part, after_file)) = split_win_safe_colon(loc_str.trim()) else {
        return Ok(None);
    };
    let mut nums = after_file.split(':');
    let line = nums
        .next()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let column = nums
        .next()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Taplo,
        path: normalize_tool_path_with_fallback(
            file_part.trim(),
            workspace_root,
            ExternalToolKind::Taplo,
            "Cargo.toml",
        )?,
        line,
        column,
        native_code: header.0,
        message: header.1,
        remediation: String::from("Fix the TOML syntax or schema violation reported by `taplo`."),
    }))
}

/// Parses `taplo check` / `taplo lint` (`codespan_reporting` or single-line) output (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if a normalized repository path is invalid.
pub fn parse_taplo_output(
    raw_text: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out = Vec::new();
    let mut pending_header: Option<(String, String)> = None;

    for raw_line in raw_text.lines() {
        let trimmed = raw_line.trim();
        if let Some(hdr) = parse_taplo_header(trimmed) {
            pending_header = Some(hdr);
            continue;
        }
        if let Some(loc_str) = trimmed
            .strip_prefix("┌─ ")
            .or_else(|| trimmed.strip_prefix("--> "))
            && let Some(header) = pending_header.take()
            && let Some(finding) = build_taplo_finding(loc_str, header, workspace_root)?
        {
            out.push(finding);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Parses `xmllint --noout` stderr/stdout diagnostics (`path:line: category : msg`, Windows-safe).
///
/// # Errors
///
/// Returns [`ToolParseError`] if a normalized repository path is invalid.
pub fn parse_xmllint_output(
    raw_text: &str,
    workspace_root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out = Vec::new();
    for raw_line in raw_text.lines() {
        let trimmed = raw_line.trim();
        let Some((file_part, rest)) = split_win_safe_colon(trimmed) else {
            continue;
        };
        let Some((line_str, detail)) = rest.split_once(':') else {
            continue;
        };
        let Ok(line) = line_str.trim().parse::<u32>() else {
            continue;
        };
        let msg = detail.trim();
        if msg.is_empty() {
            continue;
        }
        out.push(ExternalToolFinding {
            tool: ExternalToolKind::Xmllint,
            path: normalize_tool_path_with_fallback(
                file_part.trim(),
                workspace_root,
                ExternalToolKind::Xmllint,
                "pom.xml",
            )?,
            line: line.max(1),
            column: 1,
            native_code: String::from("xmllint"),
            message: String::from(msg),
            remediation: String::from(
                "Fix the XML well-formedness or entity error reported by `xmllint`.",
            ),
        });
    }
    out.sort();
    out.dedup();
    Ok(out)
}
