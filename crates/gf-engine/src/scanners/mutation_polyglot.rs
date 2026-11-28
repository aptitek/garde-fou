//! Polyglot mutation-report parsers for Go `gremlins` JSON, Python `mutmut` JSON,
//! and Kotlin/JVM `PITest` XML (`GF052`).

use std::path::Path;

use gf_core::{MutationSummary, Span, SurvivedMutant, truncate_replacement};
use serde::Deserialize;

use super::sarif_ingest::normalize_artifact_uri;
use crate::types::EngineError;

#[derive(Deserialize)]
struct GremlinsMutation {
    line: Option<u32>,
    #[serde(rename = "type")]
    mutator_type: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct GremlinsFile {
    file_name: Option<String>,
    #[serde(default)]
    mutations: Vec<GremlinsMutation>,
}

#[derive(Deserialize)]
struct GremlinsReport {
    #[serde(default)]
    files: Vec<GremlinsFile>,
}

#[derive(Deserialize)]
struct MutmutItem {
    filename: Option<String>,
    line_number: Option<u32>,
    status: Option<String>,
    mutator: Option<String>,
    replacement: Option<String>,
    function_name: Option<String>,
}

#[derive(Deserialize)]
struct MutmutReport {
    #[serde(default)]
    mutants: Vec<MutmutItem>,
}

struct SurvivorInput<'a> {
    raw_uri: Option<&'a str>,
    fallback: &'static str,
    line: u32,
    function_name: Option<String>,
    mutator: String,
    replacement: String,
}

fn push_survivor(
    out: &mut MutationSummary,
    spec: SurvivorInput<'_>,
    workspace_root: &Path,
) -> Result<(), EngineError> {
    let path = normalize_artifact_uri(spec.raw_uri, workspace_root, spec.fallback)?;
    let ln = spec.line.max(1);
    let span = Span::new(ln, ln).unwrap_or(Span::FIRST);
    out.survivors.push(SurvivedMutant {
        path,
        span,
        function_name: spec.function_name,
        mutator: spec.mutator,
        replacement: truncate_replacement(&spec.replacement),
    });
    Ok(())
}

/// Parses a Go `gremlins` JSON report (`go_module` + `files[].mutations[]`).
pub(super) fn ingest_gremlins_json(
    val: serde_json::Value,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let report: GremlinsReport = serde_json::from_value(val)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid gremlins JSON: {e}")))?;
    let mut out = MutationSummary::default();
    for file in report.files {
        let uri = file.file_name.as_deref();
        for m in file.mutations {
            match m.status.as_deref() {
                Some("KILLED") => out.caught = out.caught.saturating_add(1),
                Some("TIMED_OUT") => out.timeout = out.timeout.saturating_add(1),
                Some("NOT_VIABLE" | "SKIPPED") => out.unviable = out.unviable.saturating_add(1),
                Some("LIVED" | "NOT_COVERED") => {
                    out.missed = out.missed.saturating_add(1);
                    let mutator = m.mutator_type.unwrap_or_else(|| String::from("Gremlins"));
                    let rep = format!("gremlins `{mutator}` mutant survived");
                    let spec = SurvivorInput {
                        raw_uri: uri,
                        fallback: "go.mod",
                        line: m.line.unwrap_or(1),
                        function_name: None,
                        mutator,
                        replacement: rep,
                    };
                    push_survivor(&mut out, spec, workspace_root)?;
                }
                _ => {}
            }
        }
    }
    out.survivors.sort();
    Ok(out)
}

/// Parses a Python `mutmut` JSON report (`mutants[]`).
pub(super) fn ingest_mutmut_json(
    val: serde_json::Value,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let report: MutmutReport = serde_json::from_value(val)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid mutmut JSON: {e}")))?;
    let mut out = MutationSummary::default();
    for m in report.mutants {
        let status = m.status.as_deref().unwrap_or("").to_ascii_lowercase();
        match status.as_str() {
            "killed" => out.caught = out.caught.saturating_add(1),
            "timeout" => out.timeout = out.timeout.saturating_add(1),
            "skipped" | "not_checked" | "unviable" => out.unviable = out.unviable.saturating_add(1),
            "survived" | "suspicious" | "no_coverage" => {
                out.missed = out.missed.saturating_add(1);
                let mutator = m.mutator.unwrap_or_else(|| String::from("Mutmut"));
                let replacement = m.replacement.unwrap_or_else(|| String::from("<mutated>"));
                let spec = SurvivorInput {
                    raw_uri: m.filename.as_deref(),
                    fallback: "pyproject.toml",
                    line: m.line_number.unwrap_or(1),
                    function_name: m.function_name,
                    mutator,
                    replacement,
                };
                push_survivor(&mut out, spec, workspace_root)?;
            }
            _ => {}
        }
    }
    out.survivors.sort();
    Ok(out)
}

fn child_text<'a>(node: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Parses a Kotlin/JVM `PITest` XML report (`<mutations><mutation ...>`).
pub(super) fn ingest_pitest_xml(
    raw_xml: &str,
    workspace_root: &Path,
) -> Result<MutationSummary, EngineError> {
    let doc = roxmltree::Document::parse(raw_xml)
        .map_err(|e| EngineError::InvalidConfig(format!("invalid PITest XML: {e}")))?;
    let mut out = MutationSummary::default();
    for node in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "mutation")
    {
        match node.attribute("status") {
            Some("KILLED") => out.caught = out.caught.saturating_add(1),
            Some("TIMED_OUT") => out.timeout = out.timeout.saturating_add(1),
            Some("NON_VIABLE" | "MEMORY_ERROR" | "RUN_ERROR") => {
                out.unviable = out.unviable.saturating_add(1);
            }
            Some("SURVIVED" | "NO_COVERAGE") => {
                out.missed = out.missed.saturating_add(1);
                let line = child_text(node, "lineNumber")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let mutator = child_text(node, "mutator")
                    .unwrap_or("PITest")
                    .rsplit('.')
                    .next()
                    .unwrap_or("PITest");
                let spec = SurvivorInput {
                    raw_uri: child_text(node, "sourceFile"),
                    fallback: "build.gradle.kts",
                    line,
                    function_name: child_text(node, "mutatedMethod").map(String::from),
                    mutator: String::from(mutator),
                    replacement: String::from(
                        child_text(node, "description").unwrap_or("<mutated>"),
                    ),
                };
                push_survivor(&mut out, spec, workspace_root)?;
            }
            _ => {}
        }
    }
    out.survivors.sort();
    Ok(out)
}
