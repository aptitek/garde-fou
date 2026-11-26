//! Pure I/O-free output parsers for `astro check` (`[astro-check:<code>]`) and
//! `html-validate --formatter json` (`[html-validate:<ruleId>]` — `GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};
use serde::Deserialize;

use crate::tool_output::{ToolParseError, normalize_tool_path_with_fallback};

#[derive(Deserialize)]
#[serde(untagged)]
enum HtmlValidateSeverity {
    Num(u8),
    Str(String),
}

impl HtmlValidateSeverity {
    fn is_error(&self) -> bool {
        match self {
            Self::Num(n) => *n >= 2,
            Self::Str(s) => s.eq_ignore_ascii_case("error"),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HtmlValidateMsg {
    rule_id: Option<String>,
    severity: Option<HtmlValidateSeverity>,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HtmlValidateFileReport {
    file_path: Option<String>,
    #[serde(default)]
    messages: Vec<HtmlValidateMsg>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum HtmlValidateEnvelope {
    Bare(Vec<HtmlValidateFileReport>),
    Wrapped {
        results: Vec<HtmlValidateFileReport>,
    },
}

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some((before, after_esc)) = rest.split_once("\x1b[") {
        out.push_str(before);
        rest = after_esc
            .split_once(|c: char| c.is_ascii_alphabetic())
            .map_or("", |(_, tail)| tail);
    }
    out.push_str(rest);
    out
}

fn split_file_line_col(lhs: &str) -> Option<(&str, u32, u32)> {
    if let Some((raw_file, coords)) = lhs.rsplit_once('(')
        && let Some(coords) = coords.strip_suffix(')')
        && let Some((l_s, c_s)) = coords.split_once(',')
        && let (Ok(ln), Ok(col)) = (l_s.trim().parse::<u32>(), c_s.trim().parse::<u32>())
    {
        return Some((raw_file.trim(), ln.max(1), col.max(1)));
    }
    let (before_col, col_s) = lhs.rsplit_once(':')?;
    let (raw_file, ln_s) = before_col.rsplit_once(':')?;
    let ln = ln_s.trim().parse::<u32>().ok()?;
    let col = col_s.trim().parse::<u32>().ok()?;
    Some((raw_file.trim(), ln.max(1), col.max(1)))
}

fn parse_astro_check_line(
    line: &str,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    let clean = strip_ansi_codes(line.trim_end_matches('\r'));
    let (lhs, rhs) = if let Some(pair) = clean.split_once(" - error ") {
        pair
    } else if let Some(pair) = clean.split_once(": error ") {
        pair
    } else {
        return Ok(None);
    };
    let Some((raw_file, ln, col)) = split_file_line_col(lhs) else {
        return Ok(None);
    };
    let (code, msg) = rhs
        .split_once(':')
        .map_or_else(|| ("ts", rhs.trim()), |(c, m)| (c.trim(), m.trim()));
    let path = normalize_tool_path_with_fallback(
        raw_file,
        root,
        ExternalToolKind::AstroCheck,
        "astro.config.mjs",
    )?;
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::AstroCheck,
        path,
        line: ln,
        column: col,
        native_code: String::from(code),
        message: String::from(msg),
        remediation: String::from(
            "Fix the Astro template/frontmatter type error reported by `astro check`.",
        ),
    }))
}

/// Parses `astro check` diagnostic output into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] only if a normalized repository path is invalid.
pub fn parse_astro_check_output(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out = Vec::new();
    for line in raw.lines() {
        if let Some(finding) = parse_astro_check_line(line, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}

/// Parses `html-validate --formatter json` output into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] if `raw` is non-empty and not valid `html-validate` JSON.
pub fn parse_html_validate_json(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let env: HtmlValidateEnvelope =
        serde_json::from_str(trimmed).map_err(|e| ToolParseError::MalformedJson {
            tool: ExternalToolKind::HtmlValidate,
            detail: e.to_string(),
        })?;
    let files = match env {
        HtmlValidateEnvelope::Bare(v) | HtmlValidateEnvelope::Wrapped { results: v } => v,
    };
    let mut out = Vec::new();
    for report in files {
        let raw_file = report.file_path.as_deref().unwrap_or("index.html");
        let path = normalize_tool_path_with_fallback(
            raw_file,
            root,
            ExternalToolKind::HtmlValidate,
            "index.html",
        )?;
        for msg in report.messages {
            if !msg
                .severity
                .as_ref()
                .is_none_or(HtmlValidateSeverity::is_error)
            {
                continue;
            }
            out.push(ExternalToolFinding {
                tool: ExternalToolKind::HtmlValidate,
                path: path.clone(),
                line: msg.line.unwrap_or(1).max(1),
                column: msg.column.unwrap_or(1).max(1),
                native_code: msg.rule_id.unwrap_or_else(|| String::from("html-validate")),
                message: msg.message,
                remediation: String::from(
                    "Fix the W3C HTML5 / ARIA violation reported by `html-validate`.",
                ),
            });
        }
    }
    Ok(out)
}
