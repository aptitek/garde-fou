//! Strict `#[serde(deny_unknown_fields)]` loader for `garde-fou.toml`.

use std::fs;
use std::path::Path;

use gf_core::{
    ArchetypeSetting, ArchitectureConfig, Config, DomainLayoutConfig, FeatureId,
    ForbiddenRulesConfig, FrameworkId, LanguageId, LimitsConfig, NotebookConfig, ProjectConfig,
    ScannerPolicy, ScannersConfig, SqlConfig, SqlDialectSetting,
};

use crate::config_fortress::{
    SqlSection, merge_agents, merge_boundaries, merge_clean_code, merge_commit, merge_integrity,
    merge_tdd, merge_tools, parse_coverage_section, parse_mutation_section,
};
use crate::config_schema::{
    ArchitectureSection, DomainsSection, LimitsSection, NotebookSection, PathsAndCache,
    ProjectSection, RawManifest, RulesSection, ScannersSection, default_cache_dir,
};
use crate::types::{DEFAULT_CONFIG_FILENAME, EngineError};

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
    if section.name.as_deref().is_some_and(|n| n.trim().is_empty()) {
        return Err(EngineError::InvalidConfig(String::from(
            "`project.name` must not be empty",
        )));
    }
    let mut cfg = ProjectConfig::empty();
    if let Some(raw_arch) = section.archetype {
        cfg.archetype = ArchetypeSetting::from_config_str(&raw_arch).ok_or_else(|| {
            EngineError::InvalidConfig(format!(
                "invalid `project.archetype` `{raw_arch}` (expected `auto`, `minimal`, `scripts`, `infra`, `knowledge`, `academic`, or `software`)"
            ))
        })?;
    }
    for item in section.languages {
        cfg.languages.insert(
            LanguageId::from_slug(&item)
                .ok_or_else(|| EngineError::InvalidConfig(format!("unknown language `{item}`")))?,
        );
    }
    for item in section.features {
        cfg.features.insert(
            FeatureId::from_config_str(&item)
                .ok_or_else(|| EngineError::InvalidConfig(format!("unknown feature `{item}`")))?,
        );
    }
    for item in section.frameworks {
        cfg.frameworks
            .insert(FrameworkId::from_config_str(&item).ok_or_else(|| {
                EngineError::InvalidConfig(format!("unknown framework `{item}`"))
            })?);
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

fn merge_sql(raw: Option<SqlSection>) -> Result<SqlConfig, EngineError> {
    let base = SqlConfig::strict_defaults();
    let Some(s) = raw else {
        return Ok(base);
    };
    let dialect = match s.dialect {
        None => base.dialect,
        Some(val) => SqlDialectSetting::from_config_str(&val).ok_or_else(|| {
            EngineError::InvalidConfig(format!(
                "invalid `sql.dialect` `{val}` (expected `auto`, `postgres`, `sqlite`, `d1`, `mysql`, `duckdb`, or `generic`)"
            ))
        })?,
    };
    Ok(SqlConfig {
        dialect,
        allow_destructive_migrations: s
            .allow_destructive_migrations
            .unwrap_or(base.allow_destructive_migrations),
    })
}

fn merge_limits(raw: Option<LimitsSection>) -> Result<LimitsConfig, EngineError> {
    let base = LimitsConfig::strict_defaults();
    let Some(s) = raw else {
        return Ok(base);
    };
    if let Some(pct) = s.clone_similarity_pct
        && !(1..=100).contains(&pct)
    {
        return Err(EngineError::InvalidConfig(format!(
            "min_clone_similarity_pct {pct} must be in 1..=100"
        )));
    }
    Ok(LimitsConfig {
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
        max_struct_bools: s.struct_bools.unwrap_or(base.max_struct_bools),
        min_clone_lines: s.clone_lines.unwrap_or(base.min_clone_lines),
        min_clone_nodes: s.clone_nodes.unwrap_or(base.min_clone_nodes),
        min_clone_similarity_pct: s
            .clone_similarity_pct
            .unwrap_or(base.min_clone_similarity_pct),
    })
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

fn parse_notebook_section(raw: Option<NotebookSection>) -> NotebookConfig {
    NotebookConfig {
        require_clean_outputs: raw.and_then(|n| n.require_clean_outputs).unwrap_or(false),
    }
}

fn build_loaded_config(mut raw: RawManifest) -> Result<LoadedWorkspaceConfig, EngineError> {
    let mut analyzer = Config::strict_defaults();
    analyzer.project = parse_project_section(raw.project.take())?;
    analyzer.limits = merge_limits(raw.limits.take())?;
    analyzer.domains = merge_domains(raw.domains.take());
    analyzer.integrity = merge_integrity(raw.integrity.take());
    analyzer.boundaries = merge_boundaries(core::mem::take(&mut raw.boundaries));
    analyzer.architecture = raw
        .architecture
        .take()
        .map_or_else(ArchitectureConfig::empty, ArchitectureSection::into_config);
    analyzer.clean_code = merge_clean_code(raw.clean_code.take());
    analyzer.tdd = merge_tdd(raw.tdd.take());
    analyzer.commit = merge_commit(raw.commit.take());
    analyzer.agents = merge_agents(raw.agents.take())?;
    analyzer.tools = merge_tools(raw.tools.take())?;
    analyzer.scanners = parse_scanners_section(raw.scanners.take())?;
    analyzer.coverage = parse_coverage_section(raw.coverage.take())?;
    analyzer.mutation = parse_mutation_section(raw.mutation.take())?;
    analyzer.rules = raw.rules.take().map_or_else(
        || Ok(ForbiddenRulesConfig::strict_defaults()),
        RulesSection::into_config,
    )?;
    analyzer.sql = merge_sql(raw.sql.take())?;
    analyzer.notebook = parse_notebook_section(raw.notebook.take());
    let pc: PathsAndCache = raw.into_paths_and_cache()?;
    analyzer.ignore_paths = pc.ignore_paths;
    analyzer.entrypoints = pc.entrypoints;
    Ok(LoadedWorkspaceConfig {
        analyzer,
        cache_enabled: pc.cache_enabled,
        cache_dir: pc.cache_dir,
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
    let raw: RawManifest =
        toml::from_str(&content).map_err(|err| EngineError::InvalidConfig(err.to_string()))?;
    if raw.schema_version != 1 {
        return Err(EngineError::InvalidConfig(format!(
            "unsupported schema_version {} (expected 1)",
            raw.schema_version
        )));
    }
    build_loaded_config(raw)
}
