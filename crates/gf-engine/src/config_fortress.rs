//! Strict `serde` TOML sections and merge helpers for fortress rules:
//! `[integrity]`, `[[boundaries]]`, `[clean_code]`, `[tdd]`, `[commit]`, `[agents]`, and `[tools]`.

use std::collections::BTreeSet;

use gf_core::{
    AgentsConfig, BoundaryRule, CleanCodeConfig, CommitConfig, IntegrityConfig, PythonToolsConfig,
    ScannerPolicy, TddConfig, TddParityGate, ToolsConfig, YagniRule, parse_extra_skill_spec,
};
use serde::Deserialize;

use crate::types::EngineError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegritySection {
    pub require_lock: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundarySection {
    pub from: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanCodeSection {
    pub max_nested_ternaries: Option<u32>,
    pub max_property_chain: Option<u32>,
    pub max_bool_parameters: Option<u32>,
    pub forbid_wildcard_exports: Option<bool>,
    pub forbid_todos: Option<bool>,
    pub forbid_single_impl_traits: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TddSection {
    pub forbid_assertion_free_tests: Option<bool>,
    pub require_companion: Option<bool>,
    pub require_stories: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitSection {
    pub allowed_types: Option<Vec<String>>,
    pub allowed_scopes: Option<Vec<String>>,
    pub max_header_len: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentsSection {
    pub sync_policy: Option<String>,
    pub remote_skills: Option<bool>,
    pub extra_skills: Option<Vec<String>>,
    pub disabled_skills: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonToolsSection {
    ruff: Option<String>,
    mypy: Option<String>,
    pyright: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageSection {
    #[serde(default)]
    pub min_line_coverage: Option<u32>,
    #[serde(default)]
    pub min_branch_coverage: Option<u32>,
    #[serde(default)]
    pub max_crap_score: Option<u32>,
    #[serde(default)]
    pub lcov_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationSection {
    #[serde(default)]
    pub min_mutation_score: Option<u32>,
    #[serde(default)]
    pub zero_survivor_globs: Vec<String>,
    #[serde(default)]
    pub report_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsSection {
    python: Option<PythonToolsSection>,
}

fn toggle_set_flag<T: Ord>(set: &mut BTreeSet<T>, flag: T, opt: Option<bool>) {
    match opt {
        Some(true) => {
            set.insert(flag);
        }
        Some(false) => {
            set.remove(&flag);
        }
        None => {}
    }
}

pub fn merge_integrity(raw: Option<IntegritySection>) -> IntegrityConfig {
    let base = IntegrityConfig::defaults();
    raw.map_or(base, |s| IntegrityConfig {
        require_lock: s.require_lock.unwrap_or(base.require_lock),
    })
}

pub fn merge_boundaries(raw: Vec<BoundarySection>) -> Vec<BoundaryRule> {
    raw.into_iter()
        .map(|b| BoundaryRule {
            from: b.from,
            allow: b.allow,
            deny: b.deny,
            reason: b.reason,
        })
        .collect()
}

pub fn merge_clean_code(raw: Option<CleanCodeSection>) -> CleanCodeConfig {
    let mut cfg = CleanCodeConfig::strict_defaults();
    let Some(s) = raw else {
        return cfg;
    };
    cfg.max_nested_ternaries = s.max_nested_ternaries.unwrap_or(cfg.max_nested_ternaries);
    cfg.max_property_chain = s.max_property_chain.unwrap_or(cfg.max_property_chain);
    cfg.max_bool_parameters = s.max_bool_parameters.unwrap_or(cfg.max_bool_parameters);
    cfg.forbid_todos = s.forbid_todos.unwrap_or(cfg.forbid_todos);
    toggle_set_flag(
        &mut cfg.yagni_rules,
        YagniRule::WildcardExports,
        s.forbid_wildcard_exports,
    );
    toggle_set_flag(
        &mut cfg.yagni_rules,
        YagniRule::SingleImplTraits,
        s.forbid_single_impl_traits,
    );
    cfg
}

pub fn merge_tdd(raw: Option<TddSection>) -> TddConfig {
    let mut cfg = TddConfig::strict_defaults();
    let Some(s) = raw else {
        return cfg;
    };
    cfg.forbid_assertion_free_tests = s
        .forbid_assertion_free_tests
        .unwrap_or(cfg.forbid_assertion_free_tests);
    toggle_set_flag(
        &mut cfg.parity_gates,
        TddParityGate::CompanionTest,
        s.require_companion,
    );
    toggle_set_flag(
        &mut cfg.parity_gates,
        TddParityGate::ComponentStories,
        s.require_stories,
    );
    cfg
}

pub fn merge_commit(raw: Option<CommitSection>) -> CommitConfig {
    let base = CommitConfig::strict_defaults();
    let Some(s) = raw else {
        return base;
    };
    CommitConfig {
        allowed_types: s.allowed_types.unwrap_or(base.allowed_types),
        allowed_scopes: s.allowed_scopes.unwrap_or(base.allowed_scopes),
        max_header_len: s.max_header_len.unwrap_or(base.max_header_len),
    }
}

pub fn merge_agents(raw: Option<AgentsSection>) -> Result<AgentsConfig, EngineError> {
    let base = AgentsConfig::defaults();
    let Some(s) = raw else {
        return Ok(base);
    };
    let sync_policy = match s.sync_policy {
        None => base.sync_policy,
        Some(val) => ScannerPolicy::from_config_str(&val).ok_or_else(|| {
            EngineError::InvalidConfig(format!(
                "invalid `agents.sync_policy` `{val}` (expected `auto`, `required`, or `off`)"
            ))
        })?,
    };
    let extra_skills = s.extra_skills.unwrap_or(base.extra_skills);
    for spec in &extra_skills {
        parse_extra_skill_spec(spec)
            .map_err(|msg| EngineError::InvalidConfig(format!("{msg}: `{spec}`")))?;
    }
    Ok(AgentsConfig {
        sync_policy,
        remote_skills: s.remote_skills.unwrap_or(base.remote_skills),
        extra_skills,
        disabled_skills: s.disabled_skills.unwrap_or(base.disabled_skills),
    })
}

fn parse_opt_tool_policy(
    raw: Option<String>,
    default: ScannerPolicy,
    field: &str,
) -> Result<ScannerPolicy, EngineError> {
    raw.map_or(Ok(default), |val| {
        ScannerPolicy::from_config_str(&val).ok_or_else(|| {
            EngineError::InvalidConfig(format!(
                "invalid `{field}` policy `{val}` (expected `auto`, `required`, or `off`)"
            ))
        })
    })
}

pub fn merge_tools(raw: Option<ToolsSection>) -> Result<ToolsConfig, EngineError> {
    let base = ToolsConfig::defaults();
    let Some(py) = raw.and_then(|t| t.python) else {
        return Ok(base);
    };
    Ok(ToolsConfig {
        python: PythonToolsConfig {
            ruff: parse_opt_tool_policy(py.ruff, base.python.ruff, "tools.python.ruff")?,
            mypy: parse_opt_tool_policy(py.mypy, base.python.mypy, "tools.python.mypy")?,
            pyright: parse_opt_tool_policy(
                py.pyright,
                base.python.pyright,
                "tools.python.pyright",
            )?,
        },
    })
}
