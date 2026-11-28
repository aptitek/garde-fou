//! Tier 2 static ecosystem tool configuration auditor (`GF081`).
//!
//! Audits `pyproject.toml`, `ruff.toml`, `.ruff.toml`, `tsconfig.json`,
//! `mutants.toml`, and `stryker.config.json` without I/O.

use gf_core::{
    CoverageFailUnder, Language, MutantsConfigIssue, PytestCoverageIssue, RepoPath,
    ToolConfigFinding, ToolConfigViolation, UastDocument, is_compiled_tool_config_file,
    is_sql_tool_config_file, is_tsconfig_filename, is_web_edge_tool_config_file,
};

use super::{
    format_tool_config::{audit_markdownlint_config, audit_stylelint_config, audit_svgo_config},
    format_tool_toml::{audit_rumdl_toml, audit_taplo_toml},
    jsonc::{audit_infection_json, audit_stryker_json, line_span_containing, push_cfg_violation},
    powershell::psd1_config::audit_psscriptanalyzer_psd1,
    shell_docker_config::{audit_hadolint_yaml, audit_shellcheckrc},
    sql::sql_tool_config::audit_sql_tool_config,
    toml_lang::analyze_toml_into_document,
    tool_config_py::{audit_mypy_table, audit_ruff_tables, section_key_span},
    ts::panda_m3e::audit_panda_config,
    ts::tsconfig_jsonc::{audit_biome_jsonc, audit_tsconfig_jsonc},
    ts::wrangler_config::audit_wrangler_config,
    yaml::yaml_tool_config::{audit_dclint_config, audit_yamllint_config, audit_zizmor_config},
};
use crate::cst::line_metrics;
use crate::types::ParseError;

const BROAD_MUTANT_EXCLUDES: &[&str] = &["*", "**", "src/**", "crates/**"];

fn addopts_disables_cov(val: &toml::Value) -> bool {
    let tokens: Vec<&str> = match val {
        toml::Value::String(s) => s.split_whitespace().collect(),
        toml::Value::Array(arr) => arr
            .iter()
            .filter_map(toml::Value::as_str)
            .flat_map(str::split_whitespace)
            .collect(),
        _ => return false,
    };
    tokens
        .iter()
        .any(|&t| t == "--no-cov" || t.starts_with("--no-cov=") || t == "no:cov" || t == "-pno:cov")
}

fn extract_fail_under_floor(val: &toml::Value) -> Option<u32> {
    if let Some(i) = val.as_integer() {
        return Some(u32::try_from(i.max(0)).unwrap_or(100));
    }
    val.as_float().map(|f| {
        let clamped = f.clamp(0.0, 100.0).floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to finite 0.0..=100.0 integer floor"
        )]
        let pct = clamped as u32;
        pct
    })
}

fn audit_pytest_and_coverage(
    tool: &toml::map::Map<String, toml::Value>,
    source: &str,
    doc: &mut UastDocument,
) {
    if let Some(ini) = tool
        .get("pytest")
        .and_then(|p| p.get("ini_options"))
        .and_then(toml::Value::as_table)
        && ini.get("addopts").is_some_and(addopts_disables_cov)
    {
        push_cfg_violation(
            &mut doc.tool_config_findings,
            ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::PytestNoCov),
            section_key_span(source, "[tool.pytest.ini_options]", "addopts"),
            String::from(
                "`--no-cov` in `[tool.pytest.ini_options] addopts` disables test coverage",
            ),
        );
    }
    if let Some(val) = tool
        .get("coverage")
        .and_then(|c| c.get("report"))
        .and_then(|r| r.get("fail_under"))
        && let Some(floor_pct) = extract_fail_under_floor(val)
    {
        doc.coverage_fail_under = Some(CoverageFailUnder {
            floor_pct,
            span: section_key_span(source, "[tool.coverage.report]", "fail_under"),
        });
    }
}

fn audit_mutants_exclude_globs(
    arr: &[toml::Value],
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    for pat in arr.iter().filter_map(toml::Value::as_str) {
        let norm = pat.trim().trim_start_matches("./");
        if BROAD_MUTANT_EXCLUDES.contains(&norm) {
            push_cfg_violation(
                out,
                ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::BroadExcludeGlobs),
                line_span_containing(source, "exclude_globs"),
                format!("broad `exclude_globs` entry `{pat}` sabotages mutation scope"),
            );
        }
    }
}

fn audit_mutants_toml(
    root_tbl: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    for (key, issue) in [
        ("examine_globs", MutantsConfigIssue::EmptyExamineGlobs),
        ("examine_re", MutantsConfigIssue::EmptyExamineRe),
    ] {
        if root_tbl
            .get(key)
            .and_then(toml::Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            push_cfg_violation(
                out,
                ToolConfigViolation::MutantsScopeWeakened(issue),
                line_span_containing(source, key),
                format!("empty {key} = [] disables all Rust mutations"),
            );
        }
    }
    if let Some(arr) = root_tbl
        .get("exclude_globs")
        .and_then(toml::Value::as_array)
    {
        audit_mutants_exclude_globs(arr, source, out);
    }
}

fn audit_toml_tool_config(path: &RepoPath, file_name: &str, source: &str, doc: &mut UastDocument) {
    analyze_toml_into_document(path, source, doc);
    let Ok(parsed) = toml::from_str::<toml::Value>(source) else {
        return;
    };
    let Some(root) = parsed.as_table() else {
        return;
    };
    if file_name == "mutants.toml" {
        audit_mutants_toml(root, source, &mut doc.tool_config_findings);
        return;
    }
    if file_name != "pyproject.toml" {
        audit_ruff_tables(root, false, source, &mut doc.tool_config_findings);
        return;
    }
    let Some(tool) = root.get("tool").and_then(toml::Value::as_table) else {
        return;
    };
    if let Some(mypy) = tool.get("mypy").and_then(toml::Value::as_table) {
        audit_mypy_table(mypy, source, &mut doc.tool_config_findings);
    }
    if let Some(ruff) = tool.get("ruff").and_then(toml::Value::as_table) {
        audit_ruff_tables(ruff, true, source, &mut doc.tool_config_findings);
    }
    audit_pytest_and_coverage(tool, source, doc);
}

#[rustfmt::skip]
fn try_audit_sql_or_web_config(path: &RepoPath, source: &str, doc: &mut UastDocument) -> bool {
    if gf_core::is_toolchain_audited_file(path.as_str()) {
        super::toolchain_modern::audit_toolchain_file(path, source, doc);
        return true;
    }
    if is_compiled_tool_config_file(path.as_str()) {
        super::tool_config_compiled::audit_compiled_tool_config(path, source, doc);
        return true;
    }
    if is_sql_tool_config_file(path) { audit_sql_tool_config(path, source, doc); return true; }
    if is_web_edge_tool_config_file(path) {
        if path.file_name().starts_with("panda.config.") { audit_panda_config(source, doc); } else { audit_wrangler_config(path, source, doc); }
        return true;
    }
    false
}

fn try_audit_format_yaml_or_web_config(
    path: &RepoPath,
    source: &str,
    doc: &mut UastDocument,
) -> bool {
    if try_audit_sql_or_web_config(path, source, doc) {
        return true;
    }
    let name = path.file_name();
    let low = name.to_ascii_lowercase();
    match low.as_str() {
        ".yamllint" | ".yamllint.yml" | ".yamllint.yaml" => audit_yamllint_config(source, doc),
        "zizmor.yml" => audit_zizmor_config(source, doc),
        "rumdl.toml" | ".rumdl.toml" => audit_rumdl_toml(path, source, doc),
        "taplo.toml" | ".taplo.toml" => audit_taplo_toml(path, source, doc),
        _ if low.starts_with(".dclintrc") => audit_dclint_config(name, source, doc),
        _ if low.starts_with(".stylelintrc") => audit_stylelint_config(name, source, doc),
        _ if low.starts_with(".markdownlint.") => audit_markdownlint_config(name, source, doc),
        _ if low.starts_with("svgo.config.") => audit_svgo_config(source, doc),
        _ => return false,
    }
    true
}

/// Extracts Tier 2 ecosystem configuration findings (`GF081`) into a [`UastDocument`].
///
/// # Errors
///
/// Returns [`ParseError`] only if span initialization fails.
pub fn extract_tool_config(
    path: &RepoPath,
    lang: Language,
    source: &str,
) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), lang, total_lines, effective_lines)?;
    if try_audit_format_yaml_or_web_config(path, source, &mut doc) {
        return Ok(doc);
    }
    match path.file_name() {
        name if is_tsconfig_filename(name) => audit_tsconfig_jsonc(name, source, &mut doc),
        name if name.eq_ignore_ascii_case("psscriptanalyzersettings.psd1") => {
            audit_psscriptanalyzer_psd1(source, &mut doc);
        }
        ".shellcheckrc" | "shellcheckrc" => {
            audit_shellcheckrc(source, &mut doc.tool_config_findings);
        }
        ".hadolint.yaml" | ".hadolint.yml" | "hadolint.yaml" | "hadolint.yml" => {
            audit_hadolint_yaml(source, &mut doc.tool_config_findings);
        }
        name @ ("biome.json" | "biome.jsonc") => audit_biome_jsonc(name, source, &mut doc),
        name @ ("stryker.config.json" | "stryker.conf.json" | "stryker-config.json") => {
            audit_stryker_json(name, source, &mut doc);
        }
        name @ ("infection.json" | "infection.json5") => {
            audit_infection_json(name, source, &mut doc);
        }
        name => audit_toml_tool_config(path, name, source, &mut doc),
    }
    Ok(doc)
}
