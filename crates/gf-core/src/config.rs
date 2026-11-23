//! Configuration, project overrides, SRP limits, and DRY clone thresholds.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::context::{FeatureId, FrameworkId, LanguageId};

/// Explicit `[project]` overrides merged with automatic workspace context detection.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Explicitly configured project languages.
    pub languages: BTreeSet<LanguageId>,
    /// Explicitly configured infrastructure features (e.g. `Docker`).
    pub features: BTreeSet<FeatureId>,
    /// Explicitly configured frameworks (e.g. `CargoWorkspace`, `Astro`).
    pub frameworks: BTreeSet<FrameworkId>,
}

impl ProjectConfig {
    /// Returns an empty set of explicit project overrides.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            languages: BTreeSet::new(),
            features: BTreeSet::new(),
            frameworks: BTreeSet::new(),
        }
    }
}

/// Quantitative SRP, complexity, and DRY clone thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    /// Maximum number of lines allowed in a single source file.
    pub max_file_lines: u32,
    /// Maximum number of effective (non-blank) lines allowed per function or method.
    pub max_function_lines: u32,
    /// Maximum parameter count allowed per callable signature.
    pub max_parameters: u32,
    /// Maximum `McCabe` cyclomatic complexity allowed per callable.
    pub max_cyclomatic_complexity: u32,
    /// Maximum cognitive complexity (including nesting penalty) allowed per callable.
    pub max_cognitive_complexity: u32,
    /// Maximum control-flow nesting depth allowed per callable.
    pub max_nesting_depth: u32,
    /// Maximum import declarations allowed in a single file (module fan-out).
    pub max_imports_per_file: u32,
    /// Maximum methods allowed on a single class, trait, or `impl` block.
    pub max_methods_per_type: u32,
    /// Minimum effective lines before two identical AST hashes trigger `GF020`.
    pub min_clone_lines: u32,
    /// Minimum AST node count before two identical AST hashes trigger `GF020`.
    pub min_clone_nodes: u32,
}

impl LimitsConfig {
    /// Returns Garde-fou's strict zero-debt dogfooding limits.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            max_file_lines: 250,
            max_function_lines: 50,
            max_parameters: 4,
            max_cyclomatic_complexity: 10,
            max_cognitive_complexity: 10,
            max_nesting_depth: 4,
            max_imports_per_file: 15,
            max_methods_per_type: 10,
            min_clone_lines: 15,
            min_clone_nodes: 40,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Top-level pure analyzer configuration consumed by `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Explicit `[project]` context overrides.
    pub project: ProjectConfig,
    /// Quantitative SRP, complexity, and DRY clone thresholds.
    pub limits: LimitsConfig,
    /// Whether forbidden constructs (`unwrap`, `panic!`, `unsafe`, etc.) are denied.
    pub forbid_panicking_constructs: bool,
    /// Explicit repository-relative glob patterns excluded from analysis.
    pub ignore_paths: Vec<String>,
    /// Explicit repository-relative glob patterns marking binary/library entrypoints.
    pub entrypoints: Vec<String>,
}

impl Config {
    /// Returns Garde-fou's strict default configuration with zero implicit exclusions.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            project: ProjectConfig::empty(),
            limits: LimitsConfig::strict_defaults(),
            forbid_panicking_constructs: true,
            ignore_paths: Vec::new(),
            entrypoints: Vec::new(),
        }
    }

    /// Produces a deterministic canonical representation suitable for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_string(&self) -> String {
        let mut ignores = self.ignore_paths.clone();
        ignores.sort();
        let mut entries = self.entrypoints.clone();
        entries.sort();
        let langs: Vec<&str> = self.project.languages.iter().map(|l| l.as_str()).collect();
        let feats: Vec<&str> = self.project.features.iter().map(|f| f.as_str()).collect();
        let fws: Vec<&str> = self
            .project
            .frameworks
            .iter()
            .map(|fw| fw.as_str())
            .collect();
        let l = self.limits;
        format!(
            "v3|fl={}|fnl={}|p={}|cyc={}|cog={}|nest={}|imp={}|meth={}|cll={}|cln={}|forbid={}|ign={}|ep={}|pl={}|pf={}|pfw={}",
            l.max_file_lines,
            l.max_function_lines,
            l.max_parameters,
            l.max_cyclomatic_complexity,
            l.max_cognitive_complexity,
            l.max_nesting_depth,
            l.max_imports_per_file,
            l.max_methods_per_type,
            l.min_clone_lines,
            l.min_clone_nodes,
            self.forbid_panicking_constructs,
            ignores.join(","),
            entries.join(","),
            langs.join(","),
            feats.join(","),
            fws.join(","),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{Config, LimitsConfig};
    use crate::FeatureId;

    #[test]
    fn canonical_string_reflects_clone_thresholds_and_project_features() {
        let base = Config::strict_defaults();
        assert_eq!(base.limits, LimitsConfig::default());
        assert_eq!(base.limits.min_clone_lines, 15);
        assert_eq!(base.limits.min_clone_nodes, 40);

        let mut modified = base.clone();
        modified.project.features.insert(FeatureId::Docker);
        modified.ignore_paths = vec![String::from("b/**"), String::from("a/**")];
        assert_ne!(base.canonical_string(), modified.canonical_string());
    }
}
