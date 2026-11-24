//! Configuration, project overrides, SRP limits, and 6-domain policies.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::boundaries::{BoundaryRule, IntegrityConfig};
use crate::clean_code::{CleanCodeConfig, TddConfig};
use crate::commit_msg::CommitConfig;
use crate::context::{FeatureId, FrameworkId, LanguageId};
use crate::domains::DomainLayoutConfig;
pub use crate::policies::{CoverageConfig, ScannerPolicy, ScannersConfig};

/// Explicit `[project]` overrides merged with automatic workspace context detection.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Explicitly configured project languages.
    pub languages: BTreeSet<LanguageId>,
    /// Explicitly configured infrastructure features (e.g. `Docker`, `Ui`, `I18n`).
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
    /// Six-domain directory layout and static asset ceiling (`[domains]`).
    pub domains: DomainLayoutConfig,
    /// Integrity lock policy (`[integrity]`, `GF070`).
    pub integrity: IntegrityConfig,
    /// Declarative architectural import boundaries (`[[boundaries]]`, `GF072`).
    pub boundaries: Vec<BoundaryRule>,
    /// KISS & YAGNI policy (`[clean_code]`, `GF073`–`GF075`).
    pub clean_code: CleanCodeConfig,
    /// TDD assertion and companion policy (`[tdd]`, `GF076`).
    pub tdd: TddConfig,
    /// Conventional Commits policy (`[commit]`, `GF077`).
    pub commit: CommitConfig,
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
    pub fn strict_defaults() -> Self {
        Self {
            project: ProjectConfig::empty(),
            limits: LimitsConfig::strict_defaults(),
            domains: DomainLayoutConfig::strict_defaults(),
            integrity: IntegrityConfig::defaults(),
            boundaries: Vec::new(),
            clean_code: CleanCodeConfig::strict_defaults(),
            tdd: TddConfig::strict_defaults(),
            commit: CommitConfig::strict_defaults(),
            scanners: ScannersConfig::defaults(),
            coverage: CoverageConfig::empty(),
            forbid_panicking_constructs: true,
            ignore_paths: Vec::new(),
            entrypoints: Vec::new(),
        }
    }

    fn fortress_fragment(&self) -> String {
        let bnd: Vec<String> = self
            .boundaries
            .iter()
            .map(BoundaryRule::canonical_fragment)
            .collect();
        format!(
            "lk={}|bnd=[{}]|{}|{}|{}",
            self.integrity.require_lock,
            bnd.join(";"),
            self.clean_code.canonical_fragment(),
            self.tdd.canonical_fragment(),
            self.commit.canonical_fragment(),
        )
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
        let dom = self.domains.canonical_fragment();
        let fort = self.fortress_fragment();
        let sec = self.scanners.secrets.as_str();
        let vul = self.scanners.vulns.as_str();
        let cov = &self.coverage;
        format!(
            "v6|fl={}|fnl={}|p={}|cyc={}|cog={}|nest={}|imp={}|meth={}|cll={}|cln={}|{dom}|{fort}|sec={sec}|vul={vul}|cov={cov:?}|f={}|i={}|e={}|l={}|ft={}|fw={}",
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
