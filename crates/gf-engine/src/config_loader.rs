//! Strict `#[serde(deny_unknown_fields)]` loader for `garde-fou.toml`.

use std::fs;
use std::path::Path;

use gf_core::{Config, LimitsConfig};
use serde::Deserialize;

use crate::{DEFAULT_CONFIG_FILENAME, EngineError};

/// Deserialized workspace configuration combining pure rules and cache settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedWorkspaceConfig {
    /// Pure analyzer configuration passed to `gf-rules`.
    pub analyzer: Config,
    /// Whether the on-disk `BLAKE3` cache is active.
    pub cache_enabled: bool,
    /// Relative directory name storing cached diagnostics.
    pub cache_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: u32,
    #[serde(default)]
    project: Option<ProjectSection>,
    #[serde(default)]
    limits: Option<LimitsSection>,
    #[serde(default)]
    rules: Option<RulesSection>,
    #[serde(default)]
    paths: Option<PathsSection>,
    #[serde(default)]
    architecture: Option<ArchitectureSection>,
    #[serde(default)]
    cache: Option<CacheSection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectSection {
    #[serde(rename = "name", default)]
    _name: Option<String>,
    #[serde(rename = "languages", default)]
    _languages: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsSection {
    #[serde(rename = "max_file_lines")]
    file_lines: Option<u32>,
    #[serde(rename = "max_function_lines")]
    function_lines: Option<u32>,
    #[serde(rename = "max_parameters")]
    parameters: Option<u32>,
    #[serde(rename = "max_cyclomatic_complexity")]
    cyclomatic_complexity: Option<u32>,
    #[serde(rename = "max_cognitive_complexity")]
    cognitive_complexity: Option<u32>,
    #[serde(rename = "max_nesting_depth")]
    nesting_depth: Option<u32>,
    #[serde(rename = "max_imports_per_file")]
    imports_per_file: Option<u32>,
    #[serde(rename = "max_methods_per_type")]
    methods_per_type: Option<u32>,
    #[serde(rename = "max_struct_bools")]
    struct_bools: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "TOML rule toggle schema mirrors explicit user configuration flags"
)]
struct RulesSection {
    #[serde(default = "default_true")]
    forbid_unsafe: bool,
    #[serde(default = "default_true")]
    forbid_unwrap: bool,
    #[serde(default = "default_true")]
    forbid_expect: bool,
    #[serde(default = "default_true")]
    forbid_panic: bool,
    #[serde(default = "default_true")]
    forbid_todo: bool,
    #[serde(default = "default_true")]
    forbid_unimplemented: bool,
    #[serde(default = "default_true")]
    forbid_dbg: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathsSection {
    #[serde(default)]
    ignore_paths: Vec<String>,
    #[serde(default)]
    entrypoints: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchitectureSection {
    #[serde(rename = "no_std_crates", default)]
    _no_std: Vec<String>,
    #[serde(rename = "io_allowed_crates", default)]
    _io_allowed: Vec<String>,
    #[serde(rename = "console_allowed_crates", default)]
    _console_allowed: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheSection {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_cache_dir")]
    directory: String,
    #[serde(default)]
    hash: Option<String>,
}

const fn default_true() -> bool {
    true
}

fn default_cache_dir() -> String {
    String::from(".garde-fou-cache")
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
    }
}

/// Loads and strictly validates `garde-fou.toml` in `root`, or returns strict defaults
/// when no manifest file is present.
///
/// # Errors
///
/// Returns [`EngineError`] if reading the file fails, if unknown TOML fields are present,
/// or if `schema_version` is unsupported.
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
    let _ = (&raw.project, &raw.architecture);

    let forbid_panicking_constructs = raw.rules.is_none_or(|r| {
        r.forbid_unsafe
            || r.forbid_unwrap
            || r.forbid_expect
            || r.forbid_panic
            || r.forbid_todo
            || r.forbid_unimplemented
            || r.forbid_dbg
    });
    let (ignore_paths, entrypoints) = raw.paths.map_or_else(
        || (Vec::new(), Vec::new()),
        |p| (p.ignore_paths, p.entrypoints),
    );
    let (cache_enabled, cache_dir) = raw.cache.map_or_else(
        || (true, default_cache_dir()),
        |c| {
            let _ = c.hash;
            (c.enabled, c.directory)
        },
    );

    Ok(LoadedWorkspaceConfig {
        analyzer: Config {
            limits: merge_limits(raw.limits),
            forbid_panicking_constructs,
            ignore_paths,
            entrypoints,
        },
        cache_enabled,
        cache_dir,
    })
}
