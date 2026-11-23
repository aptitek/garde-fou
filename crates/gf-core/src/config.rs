//! Configuration, project overrides, SRP limits, `[scanners]`, and `[coverage]` policies.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::context::{FeatureId, FrameworkId, LanguageId};

/// Policy controlling orchestration of an external security scanner (`gitleaks`, `osv-scanner`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScannerPolicy {
    /// Execute if the binary is found in `PATH`, skip silently otherwise.
    #[default]
    Auto,
    /// Fail immediately with blocking `GF043` if the binary is absent from `PATH`.
    Required,
    /// Explicitly disable this external scanner.
    Off,
}

impl ScannerPolicy {
    /// Parses a case-insensitive scanner policy (`"auto"`, `"required"`, `"off"`).
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("auto") {
            Some(Self::Auto)
        } else if raw.eq_ignore_ascii_case("required") {
            Some(Self::Required)
        } else if raw.eq_ignore_ascii_case("off") || raw.eq_ignore_ascii_case("false") {
            Some(Self::Off)
        } else {
            None
        }
    }

    /// Canonical lowercase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Required => "required",
            Self::Off => "off",
        }
    }
}

/// External scanner policies configured under `[scanners]`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannersConfig {
    /// Policy for `gitleaks` (`GF041`).
    pub secrets: ScannerPolicy,
    /// Policy for `osv-scanner` (`GF042`).
    pub vulns: ScannerPolicy,
}

impl ScannersConfig {
    /// Default scanner configuration (`secrets = "auto"`, `vulns = "auto"`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            secrets: ScannerPolicy::Auto,
            vulns: ScannerPolicy::Auto,
        }
    }
}

/// Universal `LCOV` coverage gate configured under `[coverage]`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageConfig {
    /// Minimum required line-coverage percentage (`0..=100`), enforced by `GF050`.
    pub min_line_coverage: Option<u32>,
    /// Repository-relative path to the `LCOV` file (defaults to `"lcov.info"` when active).
    pub lcov_path: Option<String>,
}

impl CoverageConfig {
    /// Empty coverage gate (disabled unless `min_line_coverage` or `--lcov` is set).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            min_line_coverage: None,
            lcov_path: None,
        }
    }
}

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

/// Top-level pure analyzer configuration consumed by `gf-rules` and `gf-engine`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Explicit `[project]` context overrides.
    pub project: ProjectConfig,
    /// Quantitative SRP, complexity, and DRY clone thresholds.
    pub limits: LimitsConfig,
    /// External scanner policies (`gitleaks`, `osv-scanner`).
    pub scanners: ScannersConfig,
    /// Optional `LCOV` coverage gate (`GF050`).
    pub coverage: CoverageConfig,
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
            scanners: ScannersConfig::defaults(),
            coverage: CoverageConfig::empty(),
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
            "v4|fl={}|fnl={}|p={}|cyc={}|cog={}|nest={}|imp={}|meth={}|cll={}|cln={}|sec={}|vul={}|cov={:?}|lcov={:?}|forbid={}|ign={}|ep={}|pl={}|pf={}|pfw={}",
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
            self.scanners.secrets.as_str(),
            self.scanners.vulns.as_str(),
            self.coverage.min_line_coverage,
            self.coverage.lcov_path,
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
