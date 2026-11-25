//! Static `.svg` security analyzer (`GF064`) and process-invocation script-mention
//! collector (`GF065`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{RepoPath, UastDocument};

use super::xml::extract_svg_roxml;
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

/// Parses an `.svg` file via `roxmltree` and inspects it for XXE, `<script>`, SMIL, and `data:` hazards (`GF064`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document span fails.
pub fn extract_svg_asset(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    extract_svg_roxml(path, source)
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
