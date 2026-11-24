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
use crate::mutation::MutationConfig;
pub use crate::policies::{
    ArchitectureConfig, CoverageConfig, ForbiddenRuleKind, ForbiddenRulesConfig, ScannerPolicy,
    ScannersConfig,
};
use crate::skills_catalog::AgentsConfig;
use crate::tool_ingest::ToolsConfig;

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

/// Quantitative SRP, complexity, struct-boolean, and DRY clone thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    /// Maximum number of lines allowed in a single source file (`GF001`).
    pub max_file_lines: u32,
    /// Maximum number of effective (non-blank) lines allowed per function or method (`GF002`).
    pub max_function_lines: u32,
    /// Maximum parameter count allowed per callable signature (`GF003`).
    pub max_parameters: u32,
    /// Maximum `McCabe` cyclomatic complexity allowed per callable (`GF005`).
    pub max_cyclomatic_complexity: u32,
    /// Maximum cognitive complexity (including nesting penalty) allowed per callable (`GF004`).
    pub max_cognitive_complexity: u32,
    /// Maximum control-flow nesting depth allowed per callable (`GF006`).
    pub max_nesting_depth: u32,
    /// Maximum import declarations allowed in a single file (`GF007`).
    pub max_imports_per_file: u32,
    /// Maximum methods allowed on a single class, trait, or `impl` block (`GF008`).
    pub max_methods_per_type: u32,
    /// Maximum direct `bool` fields allowed on a single `struct` / `class` (`GF009`).
    pub max_struct_bools: u32,
    /// Minimum effective lines before AST clone detection triggers (`GF020`).
    pub min_clone_lines: u32,
    /// Minimum AST node count before AST clone detection triggers (`GF020`).
    pub min_clone_nodes: u32,
    /// Minimum `MinHash` Jaccard similarity percentage (`0..=100`) for Type-3 clones (`GF020`).
    pub min_clone_similarity_pct: u32,
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
            max_struct_bools: 2,
            min_clone_lines: 15,
            min_clone_nodes: 40,
            min_clone_similarity_pct: 85,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Top-level pure analyzer configuration consumed by `gf-rules` and `gf-engine` (zero booleans).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Explicit `[project]` context overrides.
    pub project: ProjectConfig,
    /// Quantitative SRP, complexity, struct-bool, and DRY clone thresholds.
    pub limits: LimitsConfig,
    /// Six-domain directory layout and static asset ceiling (`[domains]`).
    pub domains: DomainLayoutConfig,
    /// Integrity lock policy (`[integrity]`, `GF070`).
    pub integrity: IntegrityConfig,
    /// Declarative architectural import boundaries (`[[boundaries]]`, `GF072`).
    pub boundaries: Vec<BoundaryRule>,
    /// Crate effect-isolation and `#![no_std]` policy (`[architecture]`, `GF078`).
    pub architecture: ArchitectureConfig,
    /// KISS & YAGNI policy (`[clean_code]`, `GF073`–`GF075`).
    pub clean_code: CleanCodeConfig,
    /// TDD assertion and companion policy (`[tdd]`, `GF076`).
    pub tdd: TddConfig,
    /// Conventional Commits policy (`[commit]`, `GF077`).
    pub commit: CommitConfig,
    /// Agentic `AGENTS.md` & `skills.sh` synchronization policy (`[agents]`, `GF070`).
    pub agents: AgentsConfig,
    /// Language ecosystem tool adapter policies (`[tools]`, `GF082` / `GF043`).
    pub tools: ToolsConfig,
    /// External scanner policies (`gitleaks`, `osv-scanner`).
    pub scanners: ScannersConfig,
    /// Optional `LCOV` line/branch coverage and `CRAP` gate (`GF050`–`GF051`).
    pub coverage: CoverageConfig,
    /// Optional mutation-testing score and zero-survivor gate (`[mutation]`, `GF052`).
    pub mutation: MutationConfig,
    /// Granular `[rules]` set of forbidden constructs (`GF010`–`GF011`).
    pub rules: ForbiddenRulesConfig,
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
            architecture: ArchitectureConfig::empty(),
            clean_code: CleanCodeConfig::strict_defaults(),
            tdd: TddConfig::strict_defaults(),
            commit: CommitConfig::strict_defaults(),
            agents: AgentsConfig::defaults(),
            tools: ToolsConfig::defaults(),
            scanners: ScannersConfig::defaults(),
            coverage: CoverageConfig::empty(),
            mutation: MutationConfig::empty(),
            rules: ForbiddenRulesConfig::strict_defaults(),
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
            "lk={}|bnd=[{}]|{}|{}|{}|{}|{}|{}|{}",
            self.integrity.require_lock,
            bnd.join(";"),
            self.architecture.canonical_fragment(),
            self.clean_code.canonical_fragment(),
            self.tdd.canonical_fragment(),
            self.commit.canonical_fragment(),
            self.agents.canonical_fragment(),
            self.tools.canonical_fragment(),
            self.rules.canonical_fragment(),
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
        let cov = self.coverage.canonical_fragment();
        let mut_f = self.mutation.canonical_fragment();
        format!(
            "v10|fl={}|fnl={}|p={}|cyc={}|cog={}|nest={}|imp={}|meth={}|sb={}|cll={}|cln={}|csim={}|{dom}|{fort}|sec={sec}|vul={vul}|{cov}|{mut_f}|i={}|e={}|l={}|ft={}|fw={}",
            l.max_file_lines,
            l.max_function_lines,
            l.max_parameters,
            l.max_cyclomatic_complexity,
            l.max_cognitive_complexity,
            l.max_nesting_depth,
            l.max_imports_per_file,
            l.max_methods_per_type,
            l.max_struct_bools,
            l.min_clone_lines,
            l.min_clone_nodes,
            l.min_clone_similarity_pct,
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
