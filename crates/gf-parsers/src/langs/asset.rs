//! Static `.svg` security analyzer (`GF064`) and process-invocation script-mention
//! collector (`GF065`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    AssetDocument, AssetHazard, AssetHazardKind, Language, RepoPath, Span, UastDocument,
};

use crate::cst::line_metrics;
use crate::types::ParseError;

const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "ps1", "py", "ts", "js", "mjs", "cjs"];

const PROCESS_EXEC_MARKERS: &[&str] = &[
    "Command::new",
    ".arg(",
    ".args(",
    "subprocess.",
    "Popen",
    "execFile",
    "spawn",
    "sh ",
    "bash ",
    "pwsh ",
    "pwsh\"",
    "pwsh'",
    "python",
    "tsx ",
    "node ",
];

fn parse_event_attr_at(lower_line: &str, idx: usize) -> Option<&str> {
    let bytes = lower_line.as_bytes();
    let boundary = idx == 0 || bytes[idx - 1].is_ascii_whitespace() || bytes[idx - 1] == b'<';
    if !boundary || !lower_line.get(idx..).is_some_and(|s| s.starts_with("on")) {
        return None;
    }
    let name_tail: usize = lower_line
        .get(idx + 2..)?
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    if name_tail == 0 {
        return None;
    }
    let end = idx + 2 + name_tail;
    lower_line
        .get(end..)?
        .trim_start()
        .starts_with('=')
        .then(|| lower_line.get(idx..end))
        .flatten()
}

fn has_svg_event_handler(lower_line: &str) -> Option<String> {
    (0..lower_line.len())
        .find_map(|idx| parse_event_attr_at(lower_line, idx))
        .map(String::from)
}

/// Parses an `.svg` file and inspects it for `<script>`, `on*=`, and `data:` hazards (`GF064`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document span fails.
pub fn extract_svg_asset(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), Language::Svg, total_lines, effective_lines)?;
    let mut hazards = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
        let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
        let lower = line.to_ascii_lowercase();

        if lower.contains("<script") {
            hazards.push(AssetHazard {
                kind: AssetHazardKind::SvgScriptTag,
                detail: String::from("`<script>` element inside SVG asset"),
                span,
            });
        }
        if let Some(attr) = has_svg_event_handler(&lower) {
            hazards.push(AssetHazard {
                kind: AssetHazardKind::SvgEventHandler,
                detail: format!("inline event handler `{attr}=` inside SVG asset"),
                span,
            });
        }
        if lower.contains("data:") {
            hazards.push(AssetHazard {
                kind: AssetHazardKind::SvgEmbeddedDataUri,
                detail: String::from("embedded `data:` URI inside SVG asset"),
                span,
            });
        }
    }

    doc.asset = Some(AssetDocument {
        byte_size: u64::try_from(source.len()).unwrap_or(u64::MAX),
        extension: String::from("svg"),
        hazards,
    });
    Ok(doc)
}

fn record_candidate_token(tok: &str, out: &mut BTreeSet<String>) {
    let normalized = tok.replace('\\', "/");
    let clean = normalized
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '/' | '.' | '_' | '-'))
        .trim_start_matches("./");
    if clean.is_empty() {
        return;
    }
    let file_name = clean.rsplit('/').next().unwrap_or(clean);
    let ext = file_name.rsplit_once('.').map_or("", |(_, e)| e);
    if SCRIPT_EXTENSIONS
        .iter()
        .any(|s| ext.eq_ignore_ascii_case(s))
        || clean.contains("scripts/")
        || clean.contains("tools/")
    {
        out.insert(String::from(clean));
        out.insert(String::from(file_name));
    }
}

fn scan_quoted_scripts_in_chunk(chunk: &str, out: &mut BTreeSet<String>) {
    for quote in ['"', '\'', '`'] {
        let mut inside = false;
        for segment in chunk.split(quote) {
            if inside {
                for word in segment.split_whitespace() {
                    record_candidate_token(word, out);
                }
            }
            inside = !inside;
        }
    }
}

fn has_process_exec_marker(text: &str) -> bool {
    PROCESS_EXEC_MARKERS.iter().any(|m| text.contains(m))
}

fn extract_assigned_ident(stmt: &str) -> Option<&str> {
    let lhs = stmt.split('=').next()?.trim();
    let ident = lhs
        .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find(|tok| !tok.is_empty())?;
    (!ident.starts_with('_')).then_some(ident)
}

fn chunk_mentions_ident(chunk: &str, ident: &str) -> bool {
    chunk
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| tok == ident)
}

fn record_assigned_script_binding(
    trimmed: &str,
    bound_scripts: &mut BTreeMap<String, BTreeSet<String>>,
) {
    if !trimmed.contains('=') {
        return;
    }
    let Some(ident) = extract_assigned_ident(trimmed) else {
        return;
    };
    let mut candidates = BTreeSet::new();
    scan_quoted_scripts_in_chunk(trimmed, &mut candidates);
    if !candidates.is_empty() {
        bound_scripts
            .entry(String::from(ident))
            .or_default()
            .extend(candidates);
    }
}

/// Scans process-invocation expressions in test `source` for executed script paths (`GF065`).
#[must_use]
pub fn collect_script_mentions(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut bound_scripts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut exec_chunks: Vec<&str> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        if has_process_exec_marker(trimmed) {
            scan_quoted_scripts_in_chunk(trimmed, &mut out);
            exec_chunks.push(trimmed);
        } else {
            record_assigned_script_binding(trimmed, &mut bound_scripts);
        }
    }

    for stmt in source.split(';').filter(|s| has_process_exec_marker(s)) {
        scan_quoted_scripts_in_chunk(stmt, &mut out);
        exec_chunks.push(stmt);
    }

    for (ident, scripts) in bound_scripts {
        if exec_chunks.iter().any(|c| chunk_mentions_ident(c, &ident)) {
            out.extend(scripts);
        }
    }
    out
}
