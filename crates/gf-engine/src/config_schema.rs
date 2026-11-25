//! Strict `serde` schema definitions for `garde-fou.toml`.

use std::collections::BTreeMap;

use gf_core::{ArchitectureConfig, ForbiddenRuleKind, ForbiddenRulesConfig};
use serde::Deserialize;

use crate::config_fortress::{
    AgentsSection, BoundarySection, CleanCodeSection, CommitSection, CoverageSection,
    IntegritySection, MutationSection, SqlSection, TddSection, ToolsSection,
};
use crate::types::EngineError;

/// Validated `[paths]` and `[cache]` configuration extracted from [`RawManifest`].
pub struct PathsAndCache {
    /// Repository-relative globs excluded from analysis.
    pub ignore_paths: Vec<String>,
    /// Repository-relative globs marking library/binary entrypoints.
    pub entrypoints: Vec<String>,
    /// Whether the on-disk `BLAKE3` cache is active.
    pub cache_enabled: bool,
    /// Relative directory name storing cached artifacts.
    pub cache_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub project: Option<ProjectSection>,
    #[serde(default)]
    pub limits: Option<LimitsSection>,
    #[serde(default)]
    pub domains: Option<DomainsSection>,
    #[serde(default)]
    pub integrity: Option<IntegritySection>,
    #[serde(default)]
    pub boundaries: Vec<BoundarySection>,
    #[serde(default)]
    pub architecture: Option<ArchitectureSection>,
    #[serde(default)]
    pub clean_code: Option<CleanCodeSection>,
    #[serde(default)]
    pub tdd: Option<TddSection>,
    #[serde(default)]
    pub commit: Option<CommitSection>,
    #[serde(default)]
    pub agents: Option<AgentsSection>,
    #[serde(default)]
    pub tools: Option<ToolsSection>,
    #[serde(default)]
    pub scanners: Option<ScannersSection>,
    #[serde(default)]
    pub coverage: Option<CoverageSection>,
    #[serde(default)]
    pub mutation: Option<MutationSection>,
    #[serde(default)]
    pub rules: Option<RulesSection>,
    #[serde(default)]
    pub sql: Option<SqlSection>,
    #[serde(default)]
    paths: Option<PathsSection>,
    #[serde(default)]
    cache: Option<CacheSection>,
}

impl RawManifest {
    /// Extracts and validates `[paths]` and `[cache]` (`hash` must be `"blake3"` if set).
    pub fn into_paths_and_cache(self) -> Result<PathsAndCache, EngineError> {
        let (ignore_paths, entrypoints) = self.paths.map_or_else(
            || (Vec::new(), Vec::new()),
            |p| (p.ignore_paths, p.entrypoints),
        );
        let (cache_enabled, cache_dir) = match self.cache {
            None => (true, default_cache_dir()),
            Some(c) => {
                if let Some(h) = c.hash
                    && !h.eq_ignore_ascii_case("blake3")
                {
                    return Err(EngineError::InvalidConfig(format!(
                        "unsupported cache hash `{h}` (only `blake3` is supported)"
                    )));
                }
                (c.enabled, c.directory)
            }
        };
        Ok(PathsAndCache {
            ignore_paths,
            entrypoints,
            cache_enabled,
            cache_dir,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub frameworks: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainsSection {
    pub code_globs: Option<Vec<String>>,
    pub scripts_globs: Option<Vec<String>>,
    pub forme_globs: Option<Vec<String>>,
    pub forme_token_globs: Option<Vec<String>>,
    pub texte_globs: Option<Vec<String>>,
    pub contenus_globs: Option<Vec<String>>,
    pub asset_globs: Option<Vec<String>>,
    pub max_asset_bytes: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannersSection {
    #[serde(default)]
    pub secrets: Option<String>,
    #[serde(default)]
    pub vulns: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsSection {
    #[serde(rename = "max_file_lines")]
    pub file_lines: Option<u32>,
    #[serde(rename = "max_function_lines")]
    pub function_lines: Option<u32>,
    #[serde(rename = "max_parameters")]
    pub parameters: Option<u32>,
    #[serde(rename = "max_cyclomatic_complexity")]
    pub cyclomatic_complexity: Option<u32>,
    #[serde(rename = "max_cognitive_complexity")]
    pub cognitive_complexity: Option<u32>,
    #[serde(rename = "max_nesting_depth")]
    pub nesting_depth: Option<u32>,
    #[serde(rename = "max_imports_per_file")]
    pub imports_per_file: Option<u32>,
    #[serde(rename = "max_methods_per_type")]
    pub methods_per_type: Option<u32>,
    #[serde(rename = "min_clone_lines")]
    pub clone_lines: Option<u32>,
    #[serde(rename = "min_clone_nodes")]
    pub clone_nodes: Option<u32>,
    #[serde(rename = "min_clone_similarity_pct")]
    pub clone_similarity_pct: Option<u32>,
    #[serde(rename = "max_struct_bools")]
    pub struct_bools: Option<u32>,
}

/// `[rules]` toggle map validated strictly against the 8 known `forbid_*` keys.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct RulesSection(pub BTreeMap<String, bool>);

impl RulesSection {
    /// Converts validated TOML toggles into a [`ForbiddenRulesConfig`].
    pub fn into_config(self) -> Result<ForbiddenRulesConfig, EngineError> {
        let mut cfg = ForbiddenRulesConfig::strict_defaults();
        for (key, enabled) in self.0 {
            let kind = match key.as_str() {
                "forbid_unsafe" => ForbiddenRuleKind::Unsafe,
                "forbid_unwrap" => ForbiddenRuleKind::Unwrap,
                "forbid_expect" => ForbiddenRuleKind::Expect,
                "forbid_panic" => ForbiddenRuleKind::Panic,
                "forbid_todo" => ForbiddenRuleKind::Todo,
                "forbid_unimplemented" => ForbiddenRuleKind::Unimplemented,
                "forbid_dbg" => ForbiddenRuleKind::Dbg,
                "forbid_silent_error_swallow" => ForbiddenRuleKind::SilentErrorSwallow,
                other => {
                    return Err(EngineError::InvalidConfig(format!(
                        "unknown field `{other}` in `[rules]`"
                    )));
                }
            };
            cfg.set_enabled(kind, enabled);
        }
        Ok(cfg)
    }
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
pub struct ArchitectureSection {
    #[serde(rename = "no_std_crates", default)]
    pub no_std: Vec<String>,
    #[serde(rename = "io_allowed_crates", default)]
    pub io_allowed: Vec<String>,
    #[serde(rename = "console_allowed_crates", default)]
    pub console_allowed: Vec<String>,
}

impl ArchitectureSection {
    #[must_use]
    pub fn into_config(self) -> ArchitectureConfig {
        ArchitectureConfig {
            no_std_crates: self.no_std.into_iter().collect(),
            io_allowed_crates: self.io_allowed.into_iter().collect(),
            console_allowed_crates: self.console_allowed.into_iter().collect(),
        }
    }
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

pub fn default_cache_dir() -> String {
    String::from(".garde-fou-cache")
}
