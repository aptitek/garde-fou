//! Zero-cost `--staged` target resolution for YAML, Docker Compose, and GitHub Actions
//! Tier 3 external verifiers (`yamllint`, `dclint`, `actionlint`, `zizmor`).

use std::collections::BTreeSet;

use gf_core::{
    Language, RepoPath, UastDocument, YamlFlavor, classify_yaml_flavor, is_audited_tool_config_file,
};

use super::python_runners::ExternalAdapter;

fn is_yamllint_cfg(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".yamllint" | ".yamllint.yml" | ".yamllint.yaml"
    )
}

fn is_dclint_cfg(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with(".dclintrc")
}

fn is_zizmor_cfg(name: &str) -> bool {
    name.eq_ignore_ascii_case("zizmor.yml")
}

fn resolve_filtered_yaml_docs(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    doc_pred: fn(&RepoPath) -> bool,
    cfg_pred: fn(&str) -> bool,
) -> Option<Vec<String>> {
    let all_matching: Vec<String> = docs
        .iter()
        .filter(|d| {
            d.language == Language::Yaml
                && !is_audited_tool_config_file(&d.path)
                && doc_pred(&d.path)
        })
        .map(|d| String::from(d.path.as_str()))
        .collect();
    if all_matching.is_empty() {
        return None;
    }
    let Some(staged) = staged_filter else {
        return Some(all_matching);
    };
    if staged.iter().any(|p| cfg_pred(p.file_name())) {
        return Some(all_matching);
    }
    let staged_matching: Vec<String> = all_matching
        .into_iter()
        .filter(|p| RepoPath::new(p).is_ok_and(|rp| staged.contains(&rp)))
        .collect();
    (!staged_matching.is_empty()).then_some(staged_matching)
}

const fn is_any_yaml(_path: &RepoPath) -> bool {
    true
}

fn is_compose_yaml(path: &RepoPath) -> bool {
    matches!(classify_yaml_flavor(path), YamlFlavor::DockerCompose { .. })
}

fn is_workflow_action_file(path: &RepoPath) -> bool {
    path.as_str()
        .to_ascii_lowercase()
        .strip_prefix(".github/workflows/")
        .is_some_and(|rest| !rest.contains('/'))
}

fn is_gha_yaml(path: &RepoPath) -> bool {
    classify_yaml_flavor(path) == YamlFlavor::GithubWorkflow
}

const fn no_extra_cfg(_name: &str) -> bool {
    false
}

/// Appends active `yamllint`, `dclint`, `actionlint`, and `zizmor` jobs (with `0 ms` `--staged` skip).
pub fn push_yaml_compose_ci_jobs(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    jobs: &mut Vec<(ExternalAdapter, Vec<String>)>,
) {
    if let Some(yl_targets) =
        resolve_filtered_yaml_docs(docs, staged_filter, is_any_yaml, is_yamllint_cfg)
    {
        jobs.push((ExternalAdapter::Yamllint, yl_targets));
    }
    if let Some(dc_targets) =
        resolve_filtered_yaml_docs(docs, staged_filter, is_compose_yaml, is_dclint_cfg)
    {
        jobs.push((ExternalAdapter::Dclint, dc_targets));
    }
    if let Some(al_targets) =
        resolve_filtered_yaml_docs(docs, staged_filter, is_workflow_action_file, no_extra_cfg)
    {
        jobs.push((ExternalAdapter::Actionlint, al_targets));
    }
    if let Some(zz_targets) =
        resolve_filtered_yaml_docs(docs, staged_filter, is_gha_yaml, is_zizmor_cfg)
    {
        jobs.push((ExternalAdapter::Zizmor, zz_targets));
    }
}
