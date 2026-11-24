//! Closed algebraic sum types for Tier 2 ecosystem configuration hardening (`GF081`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;

/// Specific `[tool.mypy]` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MypyConfigIssue {
    /// Missing `strict = true` or `strict = false`.
    MissingOrFalseStrict,
    /// `ignore_errors = true` in `[tool.mypy]`.
    IgnoreErrors,
    /// `disallow_untyped_defs = false` in `[tool.mypy]`.
    DisallowUntypedDefsFalse,
    /// `warn_return_any = false` in `[tool.mypy]`.
    WarnReturnAnyFalse,
    /// `[[tool.mypy.overrides]]` disabling typing on production modules.
    ProductionOverrideWeakened,
}

impl MypyConfigIssue {
    const fn key(self) -> &'static str {
        match self {
            Self::MissingOrFalseStrict => "tool.mypy.strict",
            Self::IgnoreErrors => "tool.mypy.ignore_errors",
            Self::DisallowUntypedDefsFalse => "tool.mypy.disallow_untyped_defs",
            Self::WarnReturnAnyFalse => "tool.mypy.warn_return_any",
            Self::ProductionOverrideWeakened => "tool.mypy.overrides",
        }
    }
}

/// Specific `[tool.ruff]` or standalone `ruff.toml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum RuffConfigIssue {
    /// `ignore = ["ALL"]` / `["*"]` inside `pyproject.toml` `[tool.ruff]`.
    PyprojectBlanketIgnore,
    /// `extend-ignore = ["ALL"]` / `["*"]` inside `pyproject.toml` `[tool.ruff]`.
    PyprojectBlanketExtendIgnore,
    /// `select = []` without `extend-select` inside `pyproject.toml` `[tool.ruff]`.
    PyprojectEmptySelect,
    /// `ignore = ["ALL"]` / `["*"]` inside `ruff.toml` or `.ruff.toml`.
    StandaloneBlanketIgnore,
    /// `extend-ignore = ["ALL"]` / `["*"]` inside `ruff.toml` or `.ruff.toml`.
    StandaloneBlanketExtendIgnore,
    /// `select = []` without `extend-select` inside `ruff.toml` or `.ruff.toml`.
    StandaloneEmptySelect,
}

impl RuffConfigIssue {
    const fn key(self) -> &'static str {
        match self {
            Self::PyprojectBlanketIgnore => "tool.ruff.ignore",
            Self::PyprojectBlanketExtendIgnore => "tool.ruff.extend-ignore",
            Self::PyprojectEmptySelect => "tool.ruff.select",
            Self::StandaloneBlanketIgnore => "ignore",
            Self::StandaloneBlanketExtendIgnore => "extend-ignore",
            Self::StandaloneEmptySelect => "select",
        }
    }

    const fn remediation(self) -> &'static str {
        if matches!(
            self,
            Self::PyprojectEmptySelect | Self::StandaloneEmptySelect
        ) {
            "Configure a non-empty rule set (e.g. `select = [\"E\", \"F\", \"I\", \"B\", \"UP\", \"SIM\"]`)."
        } else {
            "Remove `\"ALL\"` / `\"*\"` from `ignore` / `extend-ignore` and remediate Ruff findings at their root cause."
        }
    }
}

/// Specific `[tool.pytest.ini_options]` or `[tool.coverage.report]` weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PytestCoverageIssue {
    /// `--no-cov` or `no:cov` in `[tool.pytest.ini_options] addopts`.
    PytestNoCov,
    /// `[tool.coverage.report] fail_under` below configured `min_line_coverage`.
    CoverageFailUnderTooLow,
}

impl PytestCoverageIssue {
    const fn tool(self) -> ExternalToolKind {
        match self {
            Self::PytestNoCov => ExternalToolKind::Pytest,
            Self::CoverageFailUnderTooLow => ExternalToolKind::Coverage,
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::PytestNoCov => "tool.pytest.ini_options.addopts",
            Self::CoverageFailUnderTooLow => "tool.coverage.report.fail_under",
        }
    }

    const fn remediation(self) -> &'static str {
        match self {
            Self::PytestNoCov => {
                "Remove `--no-cov` / `-p no:cov` from `[tool.pytest.ini_options] addopts`."
            }
            Self::CoverageFailUnderTooLow => {
                "Raise `[tool.coverage.report] fail_under` to at least `min_line_coverage` in `garde-fou.toml`."
            }
        }
    }
}

/// Specific `tsconfig.json` strictness weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TsconfigIssue {
    /// Missing `"strict": true` or `"strict": false`.
    MissingOrFalseStrict,
    /// `"noImplicitAny": false`.
    NoImplicitAnyFalse,
    /// `"strictNullChecks": false`.
    StrictNullChecksFalse,
}

impl TsconfigIssue {
    const fn key(self) -> &'static str {
        match self {
            Self::MissingOrFalseStrict => "compilerOptions.strict",
            Self::NoImplicitAnyFalse => "compilerOptions.noImplicitAny",
            Self::StrictNullChecksFalse => "compilerOptions.strictNullChecks",
        }
    }
}

/// Specific `mutants.toml` scope sabotage (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MutantsConfigIssue {
    /// `examine_globs = []`.
    EmptyExamineGlobs,
    /// `examine_re = []`.
    EmptyExamineRe,
    /// Broad wildcard `exclude_globs` entry (`*`, `**`, `src/**`, `crates/**`).
    BroadExcludeGlobs,
}

impl MutantsConfigIssue {
    const fn key(self) -> &'static str {
        match self {
            Self::EmptyExamineGlobs => "examine_globs",
            Self::EmptyExamineRe => "examine_re",
            Self::BroadExcludeGlobs => "exclude_globs",
        }
    }
}

/// Specific `stryker.config.json` mutation gate weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum StrykerConfigIssue {
    /// `"mutate": []`.
    EmptyMutate,
    /// `"thresholds.break": 0` or `null`.
    DisabledBreakThreshold,
}

impl StrykerConfigIssue {
    const fn key(self) -> &'static str {
        match self {
            Self::EmptyMutate => "mutate",
            Self::DisabledBreakThreshold => "thresholds.break",
        }
    }
}

/// Closed sum type of Tier 2 ecosystem tool configuration weakenings (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ToolConfigViolation {
    /// `[tool.mypy]` without `strict = true` or weakened overrides (`MypyNotStrict`).
    MypyNotStrict(MypyConfigIssue),
    /// `[tool.ruff]` / `ruff.toml` blanket ignore or empty select (`RuffWeakened`).
    RuffWeakened(RuffConfigIssue),
    /// `[tool.pytest.ini_options]` `--no-cov` or lowered `[tool.coverage.report]` `fail_under`.
    PytestOrCoverageWeakened(PytestCoverageIssue),
    /// `tsconfig.json` strictness disabled.
    TsconfigNotStrict(TsconfigIssue),
    /// `mutants.toml` mutation scope sabotaged.
    MutantsScopeWeakened(MutantsConfigIssue),
    /// `stryker.config.json` mutation threshold disabled.
    StrykerWeakened(StrykerConfigIssue),
    /// `biome.json` / `biome.jsonc` disabling `"linter": { "enabled": false }`.
    BiomeLinterDisabled,
}

impl ToolConfigViolation {
    /// Ecosystem tool affected by this configuration violation.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::MypyNotStrict(_) => ExternalToolKind::Mypy,
            Self::RuffWeakened(_) => ExternalToolKind::Ruff,
            Self::PytestOrCoverageWeakened(k) => k.tool(),
            Self::TsconfigNotStrict(_) => ExternalToolKind::Tsc,
            Self::MutantsScopeWeakened(_) => ExternalToolKind::CargoMutants,
            Self::StrykerWeakened(_) => ExternalToolKind::Stryker,
            Self::BiomeLinterDisabled => ExternalToolKind::Biome,
        }
    }

    /// Canonical dotted key path inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::MypyNotStrict(k) => k.key(),
            Self::RuffWeakened(k) => k.key(),
            Self::PytestOrCoverageWeakened(k) => k.key(),
            Self::TsconfigNotStrict(k) => k.key(),
            Self::MutantsScopeWeakened(k) => k.key(),
            Self::StrykerWeakened(k) => k.key(),
            Self::BiomeLinterDisabled => "linter.enabled",
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::MypyNotStrict(_) => REM_CFG_MYPY,
            Self::RuffWeakened(k) => k.remediation(),
            Self::PytestOrCoverageWeakened(k) => k.remediation(),
            Self::TsconfigNotStrict(_) => REM_CFG_TSCONFIG,
            Self::MutantsScopeWeakened(_) => REM_CFG_MUTANTS,
            Self::StrykerWeakened(_) => REM_CFG_STRYKER,
            Self::BiomeLinterDisabled => REM_CFG_BIOME,
        }
    }
}

const REM_CFG_MYPY: &str = "Enforce `strict = true` in `[tool.mypy]` without `ignore_errors = true` or weakened production overrides.";
const REM_CFG_TSCONFIG: &str = "Enforce `\"strict\": true` inside `\"compilerOptions\"` in `tsconfig.json` without `false` sub-flag overrides.";
const REM_CFG_MUTANTS: &str = "Keep `examine_globs` non-empty and never exclude entire `src/**` or `crates/**` trees in `mutants.toml`.";
const REM_CFG_STRYKER: &str = "Configure non-empty `\"mutate\"` globs and a positive `\"thresholds.break\"` score in `stryker.config.json`.";
const REM_CFG_BIOME: &str =
    "Keep `\"linter\": { \"enabled\": true }` active in `biome.json` / `biome.jsonc`.";

impl fmt::Display for ToolConfigViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.tool(), self.key())
    }
}
