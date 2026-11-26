//! Closed algebraic sum types for Tier 2 ecosystem configuration hardening (`GF081`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::compiled_tools::CompiledConfigIssue;
use crate::format_tools::FormatConfigIssue;
use crate::language_meta::ExternalToolKind;
pub use crate::mutation_config_issues::{MutantsConfigIssue, StrykerConfigIssue};
use crate::shell_ps_tools::ShellInfraConfigIssue;
use crate::sql_tools::SqlToolConfigIssue;
use crate::toolchain_mern_violations::ToolchainViolation;
use crate::web_edge_tools::WebEdgeConfigIssue;
use crate::yaml_tools::YamlCiConfigIssue;

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
        const KEYS: [&str; 5] = [
            "tool.mypy.strict",
            "tool.mypy.ignore_errors",
            "tool.mypy.disallow_untyped_defs",
            "tool.mypy.warn_return_any",
            "tool.mypy.overrides",
        ];
        KEYS[self as usize]
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
        const KEYS: [&str; 6] = [
            "tool.ruff.ignore",
            "tool.ruff.extend-ignore",
            "tool.ruff.select",
            "ignore",
            "extend-ignore",
            "select",
        ];
        KEYS[self as usize]
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

/// Closed sum type of Tier 2 ecosystem tool configuration weakenings (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ToolConfigViolation {
    /// `[tool.mypy]` without `strict = true` or weakened overrides.
    MypyNotStrict(MypyConfigIssue),
    /// `[tool.ruff]` / `ruff.toml` blanket ignore or empty select.
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
    /// `.shellcheckrc`, `.hadolint.yaml`, or `PSScriptAnalyzerSettings.psd1` weakened.
    ShellInfra(ShellInfraConfigIssue),
    /// `.yamllint*`, `.dclintrc*`, or `zizmor.yml` weakened.
    YamlCi(YamlCiConfigIssue),
    /// `.stylelintrc*`, `.markdownlint*`/`rumdl.toml`, `taplo.toml`, or `svgo.config.*` weakened.
    Formats(FormatConfigIssue),
    /// `.squawk.toml`, `.sqruff`, `.sqlfluff`, or `drizzle.config.*` weakened.
    Sql(SqlToolConfigIssue),
    /// `panda.config.*` or `wrangler.{toml,json,jsonc}` weakened.
    WebEdge(WebEdgeConfigIssue),
    /// Go (`go.mod`/`.golangci.yml`), Kotlin (`detekt.yml`/`build.gradle.kts`), or C# (`*.csproj`/`.editorconfig`) weakened.
    Compiled(CompiledConfigIssue),
    /// Modern exclusive toolchain violation (`pnpm`, `uv`, Gradle `.kts`, Node ESM `"type": "module"`).
    Toolchain(ToolchainViolation),
    /// PHP (`composer.json`, `phpstan.neon`, `psalm.xml`) configuration weakened.
    Php(crate::php_tools::PhpConfigIssue),
    /// Dart / Flutter (`pubspec.yaml`, `analysis_options.yaml`) configuration weakened.
    Dart(crate::dart_tools::DartConfigIssue),
}

impl ToolConfigViolation {
    const fn domain_descriptor(self) -> Option<(ExternalToolKind, &'static str, &'static str)> {
        match self {
            Self::ShellInfra(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::YamlCi(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::Formats(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::Sql(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::WebEdge(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::Compiled(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::Toolchain(k) => Some((k.tool(), k.code(), k.remediation())),
            Self::Php(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::Dart(k) => Some((k.tool(), k.key(), k.remediation())),
            Self::MypyNotStrict(_)
            | Self::RuffWeakened(_)
            | Self::PytestOrCoverageWeakened(_)
            | Self::TsconfigNotStrict(_)
            | Self::MutantsScopeWeakened(_)
            | Self::StrykerWeakened(_)
            | Self::BiomeLinterDisabled => None,
        }
    }

    const fn descriptor(self) -> (ExternalToolKind, &'static str, &'static str) {
        if let Some(triple) = self.domain_descriptor() {
            return triple;
        }
        match self {
            Self::MypyNotStrict(k) => (ExternalToolKind::Mypy, k.key(), REM_CFG_MYPY),
            Self::RuffWeakened(k) => (ExternalToolKind::Ruff, k.key(), k.remediation()),
            Self::PytestOrCoverageWeakened(k) => (k.tool(), k.key(), k.remediation()),
            Self::TsconfigNotStrict(k) => (ExternalToolKind::Tsc, k.key(), REM_CFG_TSCONFIG),
            Self::MutantsScopeWeakened(k) => {
                (ExternalToolKind::CargoMutants, k.key(), REM_CFG_MUTANTS)
            }
            Self::StrykerWeakened(k) => (ExternalToolKind::Stryker, k.key(), REM_CFG_STRYKER),
            _ => (ExternalToolKind::Biome, "linter.enabled", REM_CFG_BIOME),
        }
    }

    /// Ecosystem tool affected by this configuration violation.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        self.descriptor().0
    }

    /// Canonical dotted key path inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        self.descriptor().1
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        self.descriptor().2
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
