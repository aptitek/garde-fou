//! Strict `#[serde(deny_unknown_fields)]` loader for `garde-fou.toml`.

use std::fs;
use std::path::Path;

use gf_core::{
    Config, CoverageConfig, DomainLayoutConfig, FeatureId, FrameworkId, LanguageId, LimitsConfig,
    ProjectConfig, ScannerPolicy, ScannersConfig,
};

use crate::config_schema::{
    CoverageSection, DomainsSection, LimitsSection, ProjectSection, RawManifest, RulesSection,
    ScannersSection, default_cache_dir,
};
use crate::{DEFAULT_CONFIG_FILENAME, EngineError};

/// Deserialized workspace configuration combining pure rules and cache settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedWorkspaceConfig {
    /// Pure analyzer configuration passed to `gf-rules` and external scanners.
    pub analyzer: Config,
    /// Whether the on-disk `BLAKE3` cache is active.
    pub cache_enabled: bool,
    /// Relative directory name storing cached artifacts.
    pub cache_dir: String,
}

fn parse_project_section(raw: Option<ProjectSection>) -> Result<ProjectConfig, EngineError> {
    let Some(section) = raw else {
        return Ok(ProjectConfig::empty());
    };
    let mut cfg = ProjectConfig::empty();
    for item in section.languages {
        let id = LanguageId::from_config_str(&item)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown language `{item}`")))?;
        cfg.languages.insert(id);
    }
    for item in section.features {
        let id = FeatureId::from_config_str(&item)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown feature `{item}`")))?;
        cfg.features.insert(id);
    }
    for item in section.frameworks {
        let id = FrameworkId::from_config_str(&item)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown framework `{item}`")))?;
        cfg.frameworks.insert(id);
    }
    Ok(cfg)
}

fn parse_policy_field(field_name: &str, raw: Option<String>) -> Result<ScannerPolicy, EngineError> {
    let Some(val) = raw else {
        return Ok(ScannerPolicy::Auto);
    };
    ScannerPolicy::from_config_str(&val).ok_or_else(|| {
        EngineError::InvalidConfig(format!(
            "invalid scanner policy `{val}` for `{field_name}` (expected `auto`, `required`, or `off`)"
        ))
    })
}

fn parse_scanners_section(raw: Option<ScannersSection>) -> Result<ScannersConfig, EngineError> {
    let Some(sec) = raw else {
        return Ok(ScannersConfig::defaults());
    };
    Ok(ScannersConfig {
        secrets: parse_policy_field("secrets", sec.secrets)?,
        vulns: parse_policy_field("vulns", sec.vulns)?,
    })
}

fn parse_coverage_section(raw: Option<CoverageSection>) -> Result<CoverageConfig, EngineError> {
    let Some(cov) = raw else {
        return Ok(CoverageConfig::empty());
    };
    if let Some(pct) = cov.min_line_coverage
        && pct > 100
    {
        return Err(EngineError::InvalidConfig(format!(
            "min_line_coverage {pct} exceeds 100%"
        )));
    }
    Ok(CoverageConfig {
        min_line_coverage: cov.min_line_coverage,
        lcov_path: cov.lcov_path,
    })
}

fn merge_limits(raw: Option<LimitsSection>) -> LimitsConfig {
    let base = LimitsConfig::strict_defaults();
    let Some(s) = raw else {
        return base;
    };
    let _ = s.struct_bools;
    LimitsConfig {
        max_file_lines: s.file_lines.unwrap_or(base.max_file_lines),
        max_function_lines: s.function_lines.unwrap_or(base.max_function_lines),
        max_parameters: s.parameters.unwrap_or(base.max_parameters),
        max_cyclomatic_complexity: s
            .cyclomatic_complexity
            .unwrap_or(base.max_cyclomatic_complexity),
        max_cognitive_complexity: s
            .cognitive_complexity
            .unwrap_or(base.max_cognitive_complexity),
        max_nesting_depth: s.nesting_depth.unwrap_or(base.max_nesting_depth),
        max_imports_per_file: s.imports_per_file.unwrap_or(base.max_imports_per_file),
        max_methods_per_type: s.methods_per_type.unwrap_or(base.max_methods_per_type),
        min_clone_lines: s.clone_lines.unwrap_or(base.min_clone_lines),
        min_clone_nodes: s.clone_nodes.unwrap_or(base.min_clone_nodes),
    }
}

fn merge_domains(raw: Option<DomainsSection>) -> DomainLayoutConfig {
    let base = DomainLayoutConfig::strict_defaults();
    let Some(d) = raw else {
        return base;
    };
    DomainLayoutConfig {
        code_globs: d.code_globs.unwrap_or(base.code_globs),
        scripts_globs: d.scripts_globs.unwrap_or(base.scripts_globs),
        forme_globs: d.forme_globs.unwrap_or(base.forme_globs),
        forme_token_globs: d.forme_token_globs.unwrap_or(base.forme_token_globs),
        texte_globs: d.texte_globs.unwrap_or(base.texte_globs),
        contenus_globs: d.contenus_globs.unwrap_or(base.contenus_globs),
        asset_globs: d.asset_globs.unwrap_or(base.asset_globs),
        max_asset_bytes: d.max_asset_bytes.unwrap_or(base.max_asset_bytes),
    }
}

fn forbid_constructs_enabled(rules: Option<RulesSection>) -> bool {
    rules.is_none_or(|r| {
        r.forbid_unsafe
            || r.forbid_unwrap
            || r.forbid_expect
            || r.forbid_panic
            || r.forbid_todo
            || r.forbid_unimplemented
            || r.forbid_dbg
    })
}

/// Loads and strictly validates `garde-fou.toml` in `root`, or returns strict defaults.
///
/// # Errors
///
/// Returns [`EngineError`] on I/O failure, unknown TOML fields, or invalid identifiers.
pub fn load_workspace_config(root: &Path) -> Result<LoadedWorkspaceConfig, EngineError> {
    let path = root.join(DEFAULT_CONFIG_FILENAME);
    if !path.is_file() {
        return Ok(LoadedWorkspaceConfig {
            analyzer: Config::strict_defaults(),
            cache_enabled: true,
            cache_dir: default_cache_dir(),
        });
    }

    let content = fs::read_to_string(&path)?;
    let mut raw: RawManifest =
        toml::from_str(&content).map_err(|err| EngineError::InvalidConfig(err.to_string()))?;
    if raw.schema_version != 1 {
        return Err(EngineError::InvalidConfig(format!(
            "unsupported schema_version {} (expected 1)",
            raw.schema_version
        )));
    }
    let project = parse_project_section(raw.project.take())?;
    let limits = merge_limits(raw.limits.take());
    let domains = merge_domains(raw.domains.take());
    let scanners = parse_scanners_section(raw.scanners.take())?;
    let coverage = parse_coverage_section(raw.coverage.take())?;
    let forbid_panicking_constructs = forbid_constructs_enabled(raw.rules.take());
    let ((ignore_paths, entrypoints), (cache_enabled, cache_dir)) = raw.into_paths_and_cache();

    Ok(LoadedWorkspaceConfig {
        analyzer: Config {
            project,
            limits,
            domains,
            scanners,
            coverage,
            forbid_panicking_constructs,
            ignore_paths,
            entrypoints,
        },
        cache_enabled,
        cache_dir,
    })
}
