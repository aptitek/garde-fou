//! Deterministic `tsc --noEmit --pretty false` text parser and UTF-8 byte-offset line resolver (`GF082`).

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolKind};

use super::path_norm::{ToolParseError, normalize_tool_path_with_fallback};

/// Converts a 0-based UTF-8 byte offset inside `source` into 1-based `(line, column)`.
#[must_use]
pub fn byte_offset_to_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let clamped = byte_offset.min(source.len());
    let prefix = if source.is_char_boundary(clamped) {
        &source[..clamped]
    } else {
        let valid = (0..=clamped)
            .rev()
            .find(|&i| source.is_char_boundary(i))
            .unwrap_or(0);
        &source[..valid]
    };
    let mut line = 1_u32;
    let mut col = 1_u32;
    for ch in prefix.chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            col = 1;
        } else if ch != '\r' {
            col = col.saturating_add(1);
        }
    }
    (line, col)
}

fn parse_tsc_header_line(
    clean_cr: &str,
    root: &Path,
) -> Result<Option<ExternalToolFinding>, ToolParseError> {
    let Some((lhs, rhs)) = clean_cr.split_once("): error ") else {
        return Ok(None);
    };
    let Some((raw_file, coords)) = lhs.rsplit_once('(') else {
        return Ok(None);
    };
    let Some((ln_s, col_s)) = coords.split_once(',') else {
        return Ok(None);
    };
    let (Ok(line), Ok(col)) = (ln_s.trim().parse::<u32>(), col_s.trim().parse::<u32>()) else {
        return Ok(None);
    };
    let (code, message) = rhs
        .split_once(':')
        .map_or_else(|| ("TS0000", rhs.trim()), |(c, m)| (c.trim(), m.trim()));
    let path =
        normalize_tool_path_with_fallback(raw_file, root, ExternalToolKind::Tsc, "tsconfig.json")?;
    Ok(Some(ExternalToolFinding {
        tool: ExternalToolKind::Tsc,
        path,
        line: line.max(1),
        column: col.max(1),
        native_code: String::from(code),
        message: String::from(message),
        remediation: String::from(
            "Fix the TypeScript compiler error structurally without `@ts-ignore` or `any`.",
        ),
    }))
}

/// Parses `tsc --noEmit --pretty false` canonical output (including Windows `C:\...` paths
/// and indented multi-line continuation messages) into [`ExternalToolFinding`]s (`GF082`).
///
/// # Errors
///
/// Returns [`ToolParseError`] only if a normalized path fails validation.
pub fn parse_tsc_output(
    raw: &str,
    root: &Path,
) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    let mut out: Vec<ExternalToolFinding> = Vec::new();
    for raw_line in raw.lines() {
        let clean_cr = raw_line.trim_end_matches('\r');
        if clean_cr.starts_with([' ', '\t']) {
            let cont = clean_cr.trim();
            if !cont.is_empty()
                && let Some(last) = out.last_mut()
            {
                last.message.push(' ');
                last.message.push_str(cont);
            }
        } else if let Some(finding) = parse_tsc_header_line(clean_cr, root)? {
            out.push(finding);
        }
    }
    Ok(out)
}
