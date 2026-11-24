//! OASIS `SARIF` v2.1.0 ingestion, URI normalization, inline secret redaction,
//! and deterministic deduplication.

use std::collections::BTreeSet;
use std::path::Path;

use gf_core::{Diagnostic, RepoPath, Severity};

use super::sarif_schema::{SarifLog, SarifResult, SarifRun};
use crate::types::EngineError;

const SCRUB_PREFIXES: &[&str] = &[
    "ghp_",
    "github_pat_",
    "glpat-",
    "sk_live_",
    "xoxb-",
    "xoxa-",
    "xoxp-",
    "AKIA",
];

/// Target rule mapping for converting external `SARIF` results into [`Diagnostic`]s.
#[derive(Clone, Copy)]
pub struct SarifMapping<'a> {
    /// Garde-fou rule identifier (`"GF041"` for `gitleaks`, `"GF042"` for `osv-scanner`).
    pub target_rule_id: &'a str,
    /// Fallback repository-relative path when a result omits `locations`.
    pub fallback_path: &'a str,
    /// Actionable remediation hint attached to each converted diagnostic.
    pub remediation: &'a str,
}

fn strip_windows_file_slash(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'/')
        && bytes.get(1).is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(2) == Some(&b':')
    {
        &s[1..]
    } else {
        s
    }
}

pub(super) fn normalize_artifact_uri(
    uri: Option<&str>,
    workspace_root: &Path,
    fallback: &str,
) -> Result<RepoPath, EngineError> {
    let raw = uri.unwrap_or(fallback).trim();
    let unescaped = raw
        .strip_prefix("file://")
        .unwrap_or(raw)
        .replace("%20", " ")
        .replace('\\', "/");
    let without_scheme = strip_windows_file_slash(&unescaped);
    let root_str = workspace_root
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    let stripped_root = if !root_str.is_empty()
        && without_scheme.len() >= root_str.len()
        && without_scheme.is_char_boundary(root_str.len())
        && without_scheme[..root_str.len()].eq_ignore_ascii_case(&root_str)
    {
        &without_scheme[root_str.len()..]
    } else {
        without_scheme
    };
    let rel = stripped_root
        .trim_start_matches("./")
        .trim_start_matches('/');
    let candidate = if rel.is_empty() { fallback } else { rel };
    RepoPath::new(candidate)
        .or_else(|_| RepoPath::new(fallback))
        .map_err(EngineError::from)
}

fn scrub_message(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let trimmed = word.trim_matches(['"', '\'', '`', ',', ';', '(', ')']);
            SCRUB_PREFIXES
                .iter()
                .find(|&&p| trimmed.starts_with(p))
                .map_or_else(|| String::from(word), |pfx| format!("{pfx}****"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn convert_result(
    res: &SarifResult,
    run: &SarifRun,
    workspace_root: &Path,
    mapping: SarifMapping<'_>,
) -> Result<Option<Diagnostic>, EngineError> {
    let level = run.resolve_level(res);
    if level == "none" || level == "note" {
        return Ok(None);
    }
    let path = normalize_artifact_uri(res.primary_uri(), workspace_root, mapping.fallback_path)?;
    let span = res.primary_span();
    let ext_id = res.external_rule_id();
    Ok(Some(Diagnostic {
        rule_id: String::from(mapping.target_rule_id),
        severity: Severity::Error,
        message: format!("[{ext_id}] {}", scrub_message(res.message_text())),
        remediation: String::from(mapping.remediation),
        measured: 1,
        threshold: 0,
        path,
        span,
    }))
}

/// Parses a `SARIF` 2.1.0 JSON payload and converts actionable findings into [`Diagnostic`]s.
///
/// # Errors
///
/// Returns [`EngineError::InvalidConfig`] if `sarif_json` is not valid `SARIF` JSON.
pub fn ingest_sarif_json(
    sarif_json: &str,
    workspace_root: &Path,
    mapping: SarifMapping<'_>,
) -> Result<Vec<Diagnostic>, EngineError> {
    let log: SarifLog = serde_json::from_str(sarif_json)
        .map_err(|err| EngineError::InvalidConfig(format!("invalid SARIF output: {err}")))?;
    let mut seen: BTreeSet<(String, RepoPath, u32, String)> = BTreeSet::new();
    let mut out = Vec::new();

    for run in &log.runs {
        for res in &run.results {
            if let Some(diag) = convert_result(res, run, workspace_root, mapping)?
                && seen.insert((
                    diag.rule_id.clone(),
                    diag.path.clone(),
                    diag.span.start_line(),
                    diag.message.clone(),
                ))
            {
                out.push(diag);
            }
        }
    }
    Ok(out)
}
