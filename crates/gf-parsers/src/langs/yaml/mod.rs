//! Pure Safe-Rust YAML 1.2, Docker Compose, and GitHub Actions workflow extractor (`GF080`).

pub mod anchors_merge;
mod compose_extract;
mod compose_graph;
mod compose_resources;
mod compose_shell_bridge;
pub mod events_tree;
mod gha_expr;
mod gha_run_bridge;
mod gha_workflow;
pub mod yaml_ast;
pub mod yaml_quirks;
pub mod yaml_tool_config;

use gf_core::{
    AcademicSummary, Language, RepoPath, UastDocument, YamlFlavor, classify_yaml_flavor,
};

use crate::cst::line_metrics;
use crate::langs::quarto::config_audit::{collect_quarto_config_refs, is_quarto_config_path};
use crate::types::{LanguageExtractor, ParseError};
use anchors_merge::{ResolvedYaml, resolve_yaml_document};
use compose_extract::analyze_compose_services;
use compose_graph::analyze_compose_topology;
use compose_shell_bridge::bridge_compose_service_shells;
use events_tree::parse_yaml_stream;
use gha_workflow::analyze_github_workflow;
use yaml_quirks::{check_yaml_scalar_quirks, scan_yaml_comment_bypasses};

/// Pure Safe-Rust extractor for `.yml` and `.yaml` files across YAML 1.2, Compose, and GitHub Actions.
#[derive(Clone, Copy, Debug, Default)]
pub struct YamlExtractor;

impl LanguageExtractor for YamlExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Yaml, total_lines, effective_lines)?;

        let flavor = classify_yaml_flavor(path);
        let stream = parse_yaml_stream(source);
        doc.syntax_errors = stream.syntax_errors;

        let mut all_block_ranges = Vec::new();
        for raw_doc in &stream.docs {
            all_block_ranges.extend_from_slice(&raw_doc.block_body_ranges);
            check_yaml_scalar_quirks(raw_doc, flavor, &mut doc.lang_rule_findings);
            if let Some(resolved) = resolve_yaml_document(raw_doc, &mut doc.lang_rule_findings) {
                dispatch_resolved_flavor(path, flavor, &resolved, &mut doc.lang_rule_findings);
                if is_quarto_config_path(path) {
                    let q_refs = collect_quarto_config_refs(&resolved);
                    if !q_refs.is_empty() {
                        doc.academic = Some(AcademicSummary {
                            quarto_config_refs: q_refs,
                            ..AcademicSummary::default()
                        });
                    }
                }
            }
        }
        scan_yaml_comment_bypasses(source, &all_block_ranges, &mut doc.bypass_findings);
        Ok(doc)
    }
}

fn dispatch_resolved_flavor(
    path: &RepoPath,
    flavor: YamlFlavor,
    resolved: &ResolvedYaml,
    findings: &mut Vec<gf_core::LangRuleFinding>,
) {
    match flavor {
        YamlFlavor::DockerCompose { is_override } => {
            if let Some(ResolvedYaml::Mapping {
                entries: services, ..
            }) = resolved.get("services")
            {
                analyze_compose_services(services, is_override, findings);
                analyze_compose_topology(resolved, services, is_override, findings);
                bridge_compose_service_shells(services, findings);
            }
        }
        YamlFlavor::GithubWorkflow => analyze_github_workflow(path, resolved, findings),
        YamlFlavor::GenericYaml => {}
    }
}
