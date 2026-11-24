//! Strict `serde` schema definitions for `garde-fou.toml`.

use serde::Deserialize;

use crate::config_fortress::{
    BoundarySection, CleanCodeSection, CommitSection, IntegritySection, TddSection,
};

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
    pub clean_code: Option<CleanCodeSection>,
    #[serde(default)]
    pub tdd: Option<TddSection>,
    #[serde(default)]
    pub commit: Option<CommitSection>,
    #[serde(default)]
    pub scanners: Option<ScannersSection>,
    #[serde(default)]
    pub coverage: Option<CoverageSection>,
    #[serde(default)]
    pub rules: Option<RulesSection>,
    #[serde(default)]
    paths: Option<PathsSection>,
    #[serde(rename = "architecture", default)]
    _architecture: Option<ArchitectureSection>,
    #[serde(default)]
    cache: Option<CacheSection>,
}

impl RawManifest {
    pub fn into_paths_and_cache(self) -> ((Vec<String>, Vec<String>), (bool, String)) {
        let paths = self.paths.map_or_else(
            || (Vec::new(), Vec::new()),
            |p| (p.ignore_paths, p.entrypoints),
        );
        let cache = self.cache.map_or_else(
            || (true, default_cache_dir()),
            |c| {
                let _ = c.hash;
                (c.enabled, c.directory)
            },
        );
        (paths, cache)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    #[serde(rename = "name", default)]
    pub _name: Option<String>,
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
pub struct CoverageSection {
    #[serde(default)]
    pub min_line_coverage: Option<u32>,
    #[serde(default)]
    pub lcov_path: Option<String>,
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
    #[serde(rename = "max_struct_bools")]
    pub struct_bools: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "TOML rule toggle schema mirrors explicit user configuration flags"
)]
pub struct RulesSection {
    #[serde(default = "default_true")]
    pub forbid_unsafe: bool,
    #[serde(default = "default_true")]
    pub forbid_unwrap: bool,
    #[serde(default = "default_true")]
    pub forbid_expect: bool,
    #[serde(default = "default_true")]
    pub forbid_panic: bool,
    #[serde(default = "default_true")]
    pub forbid_todo: bool,
    #[serde(default = "default_true")]
    pub forbid_unimplemented: bool,
    #[serde(default = "default_true")]
    pub forbid_dbg: bool,
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

pub fn default_cache_dir() -> String {
    String::from(".garde-fou-cache")
}
