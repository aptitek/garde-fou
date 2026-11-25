//! Zero-cost `--staged` target resolution for YAML, Docker Compose, GitHub Actions,
//! CSS, Markdown, TOML, and XML/SVG Tier 3 external verifiers.

use std::collections::BTreeSet;

use gf_core::{
    Language, RepoPath, SqlDialectKind, SqlRole, UastDocument, YamlFlavor, classify_yaml_flavor,
    is_audited_tool_config_file,
};

use super::python_runners::ExternalAdapter;
use super::tool_resolvers::resolve_matching_doc_targets;

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

fn resolve_yaml_subset(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    path_pred: fn(&RepoPath) -> bool,
    cfg_pred: fn(&str) -> bool,
) -> Option<Vec<String>> {
    resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| {
            d.language == Language::Yaml
                && !is_audited_tool_config_file(&d.path)
                && path_pred(&d.path)
        },
        cfg_pred,
    )
}

/// Appends active `yamllint`, `dclint`, `actionlint`, and `zizmor` jobs (with `0 ms` `--staged` skip).
pub fn push_yaml_compose_ci_jobs(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    jobs: &mut Vec<(ExternalAdapter, Vec<String>)>,
) {
    if let Some(yl) = resolve_yaml_subset(docs, staged_filter, |_| true, is_yamllint_cfg) {
        jobs.push((ExternalAdapter::Yamllint, yl));
    }
    if let Some(dc) = resolve_yaml_subset(docs, staged_filter, is_compose_yaml, is_dclint_cfg) {
        jobs.push((ExternalAdapter::Dclint, dc));
    }
    if let Some(al) =
        resolve_yaml_subset(docs, staged_filter, is_workflow_action_file, no_extra_cfg)
    {
        jobs.push((ExternalAdapter::Actionlint, al));
    }
    if let Some(zz) = resolve_yaml_subset(docs, staged_filter, is_gha_yaml, is_zizmor_cfg) {
        jobs.push((ExternalAdapter::Zizmor, zz));
    }
}

fn is_stylelint_cfg(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with(".stylelintrc")
}

fn is_md_cfg(name: &str) -> bool {
    let low = name.to_ascii_lowercase();
    low.starts_with(".markdownlint.") || matches!(low.as_str(), "rumdl.toml" | ".rumdl.toml")
}

fn is_taplo_cfg(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "taplo.toml" | ".taplo.toml"
    )
}

/// Appends active CSS, Markdown, TOML, and XML/SVG Tier 3 jobs (with `0 ms` `--staged` skip).
pub fn push_format_jobs(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    jobs: &mut Vec<(ExternalAdapter, Vec<String>)>,
) {
    if let Some(css_t) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| d.language == Language::Css,
        is_stylelint_cfg,
    ) {
        jobs.push((ExternalAdapter::Stylelint, css_t));
    }
    if let Some(md_t) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| d.language == Language::Markdown,
        is_md_cfg,
    ) {
        jobs.push((ExternalAdapter::Rumdl, md_t.clone()));
        jobs.push((ExternalAdapter::Markdownlint, md_t));
    }
    if let Some(tm_t) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| d.language == Language::Toml,
        is_taplo_cfg,
    ) {
        jobs.push((ExternalAdapter::Taplo, tm_t));
    }
    if let Some(xml_t) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| matches!(d.language, Language::Xml | Language::Svg),
        no_extra_cfg,
    ) {
        jobs.push((ExternalAdapter::Xmllint, xml_t));
    }
    push_sql_jobs(docs, staged_filter, jobs);
}

fn is_squawk_cfg(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".squawk.toml" | "squawk.toml"
    )
}

fn is_sqruff_or_fluff_cfg(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), ".sqruff" | ".sqlfluff")
}

fn push_sql_jobs(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    jobs: &mut Vec<(ExternalAdapter, Vec<String>)>,
) {
    if let Some(pg_migs) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| {
            d.sql_doc.as_ref().is_some_and(|s| {
                s.dialect == SqlDialectKind::Postgres && matches!(s.role, SqlRole::Migration { .. })
            })
        },
        is_squawk_cfg,
    ) {
        jobs.push((ExternalAdapter::Squawk, pg_migs));
    }
    if let Some(sql_all) = resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| d.language == Language::Sql,
        is_sqruff_or_fluff_cfg,
    ) {
        jobs.push((ExternalAdapter::Sqruff, sql_all.clone()));
        jobs.push((ExternalAdapter::Sqlfluff, sql_all));
    }
}
