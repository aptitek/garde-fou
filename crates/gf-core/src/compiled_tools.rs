//! Pure `#![no_std]` Tier 2 config hardening (`CompiledConfigIssue`, `GF081`) and
//! Tier 3 tool policies (`[tools.go]`, `[tools.kotlin]`, `[tools.csharp]`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::external_tool_kind::ExternalToolKind;
use crate::policies::ScannerPolicy;

const fn default_auto() -> ScannerPolicy {
    ScannerPolicy::Auto
}

const fn default_off() -> ScannerPolicy {
    ScannerPolicy::Off
}

/// `[tools.go]` external verifier policies (`golangci-lint` v1/v2 & `go vet`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoToolsConfig {
    /// `golangci-lint` policy (`Auto` by default).
    #[serde(default = "default_auto")]
    pub golangci_lint: ScannerPolicy,
    /// `go vet -json ./...` policy (`Auto` by default).
    #[serde(default = "default_auto")]
    pub govet: ScannerPolicy,
}

impl GoToolsConfig {
    /// Strict defaults (`golangci_lint = Auto`, `govet = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            golangci_lint: ScannerPolicy::Auto,
            govet: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "go(gl={},gv={})",
            self.golangci_lint.as_str(),
            self.govet.as_str()
        )
    }
}

impl Default for GoToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.kotlin]` external verifier policies (`detekt` SARIF & `ktlint` JSON).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KotlinToolsConfig {
    /// `detekt` SARIF policy (`Auto` by default).
    #[serde(default = "default_auto")]
    pub detekt: ScannerPolicy,
    /// `ktlint` JSON policy (`Off` by default).
    #[serde(default = "default_off")]
    pub ktlint: ScannerPolicy,
}

impl KotlinToolsConfig {
    /// Strict defaults (`detekt = Auto`, `ktlint = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            detekt: ScannerPolicy::Auto,
            ktlint: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "kt(dk={},kl={})",
            self.detekt.as_str(),
            self.ktlint.as_str()
        )
    }
}

impl Default for KotlinToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.csharp]` external verifier policy (`dotnet build` Roslyn / `MSBuild`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CSharpToolsConfig {
    /// `dotnet build --no-restore -nologo -v:q` policy (`Auto` by default).
    #[serde(default = "default_auto")]
    pub dotnet_build: ScannerPolicy,
}

impl CSharpToolsConfig {
    /// Strict default (`dotnet_build = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            dotnet_build: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("cs(db={})", self.dotnet_build.as_str())
    }
}

impl Default for CSharpToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Tier 2 static configuration & manifest hardening issues for Go, Kotlin, and C# (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CompiledConfigIssue {
    /// `go.mod` contains a local relative `replace ... => ../...` directive.
    GoModLocalReplace,
    /// `go.mod` is missing an explicit `go 1.xx` toolchain version directive.
    GoModMissingVersion,
    /// `.golangci.yml` disables all linters (`disable-all: true` or `default: none`) without enabling `errcheck`/`govet`/`staticcheck`.
    GolangciDisableAll,
    /// `.golangci.yml` sets `tests: false`, skipping test analysis.
    GolangciTestsDisabled,
    /// `detekt.yml` sets `build.maxIssues` > 0.
    DetektMaxIssuesNonZero,
    /// `detekt.yml` disables `coroutines` or `exceptions` rule sets globally.
    DetektCategoryDisabled,
    /// `build.gradle.kts` / `build.gradle` uses dynamic `+` or `latest.release` dependency versions.
    GradleDynamicVersion,
    /// `build.gradle.kts` / `build.gradle` uses plaintext `http://` Maven repository or `allowInsecureProtocol = true`.
    GradleInsecureRepo,
    /// `*.csproj` or `Directory.Build.props` sets `<Nullable>disable</Nullable>`.
    CsprojNullableDisabled,
    /// `*.csproj` or `Directory.Build.props` explicitly sets `<TreatWarningsAsErrors>false</TreatWarningsAsErrors>`.
    CsprojTreatWarningsFalse,
    /// `*.csproj` or `Directory.Build.props` sets `<EnableNETAnalyzers>false</EnableNETAnalyzers>`.
    CsprojAnalyzersDisabled,
    /// `*.csproj` or `Directory.Build.props` silences critical compiler/analyzer IDs in `<NoWarn>`.
    CsprojCriticalNoWarn,
    /// `.editorconfig` globally silences `dotnet_analyzer_diagnostic.severity = none|silent`.
    EditorConfigAnalyzerSilenced,
}

impl CompiledConfigIssue {
    /// Owning [`ExternalToolKind`] for reporting.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::GoModLocalReplace | Self::GoModMissingVersion => ExternalToolKind::GoVet,
            Self::GolangciDisableAll | Self::GolangciTestsDisabled => {
                ExternalToolKind::GolangciLint
            }
            Self::DetektMaxIssuesNonZero
            | Self::DetektCategoryDisabled
            | Self::GradleDynamicVersion
            | Self::GradleInsecureRepo => ExternalToolKind::Detekt,
            Self::CsprojNullableDisabled
            | Self::CsprojTreatWarningsFalse
            | Self::CsprojAnalyzersDisabled
            | Self::CsprojCriticalNoWarn
            | Self::EditorConfigAnalyzerSilenced => ExternalToolKind::DotnetBuild,
        }
    }

    /// Canonical config key name.
    #[must_use]
    pub const fn key(self) -> &'static str {
        const KEYS: [&str; 13] = [
            "go.mod.replace",
            "go.mod.go",
            "golangci.linters",
            "golangci.run.tests",
            "detekt.build.maxIssues",
            "detekt.ruleset.active",
            "gradle.dependencies.version",
            "gradle.repositories.url",
            "csproj.Nullable",
            "csproj.TreatWarningsAsErrors",
            "csproj.EnableNETAnalyzers",
            "csproj.NoWarn",
            "editorconfig.dotnet_analyzer_diagnostic.severity",
        ];
        KEYS[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const FIXES: [&str; 13] = [
            "Remove local `replace ... => ../` directives from `go.mod` (use a local untracked `go.work` instead).",
            "Declare an explicit `go 1.xx` directive in `go.mod` for reproducible builds.",
            "Enable core Go linters (`errcheck`, `govet`, `staticcheck`) in `.golangci.yml` instead of disabling all linters.",
            "Set `tests: true` (or remove `tests: false`) in `.golangci.yml` so Go tests are also analyzed.",
            "Set `maxIssues: 0` under `build:` in `detekt.yml` to enforce zero new Kotlin violations.",
            "Keep `coroutines` and `exceptions` rule sets `active: true` in `detekt.yml`.",
            "Pin exact immutable dependency versions in `build.gradle.kts` instead of `+` or `latest.release`.",
            "Use `https://` Maven repositories and remove `allowInsecureProtocol = true` in `build.gradle.kts`.",
            "Enable nullable reference types with `<Nullable>enable</Nullable>` in `*.csproj` or `Directory.Build.props`.",
            "Remove `<TreatWarningsAsErrors>false</TreatWarningsAsErrors>` or enable `<TreatWarningsAsErrors>true</TreatWarningsAsErrors>`.",
            "Keep `<EnableNETAnalyzers>true</EnableNETAnalyzers>` enabled in `.csproj` / `Directory.Build.props`.",
            "Remove critical nullable/async IDs (`CS8618`, `CS8600`, `CS4014`, `CA2007`) from `<NoWarn>` and fix the underlying warnings.",
            "Remove global `dotnet_analyzer_diagnostic.severity = none|silent` from `.editorconfig`.",
        ];
        FIXES[self as usize]
    }
}

/// Returns `true` when `path` is a Go, Kotlin, or C#/.NET manifest or tool config file (`GF081`).
#[must_use]
pub fn is_compiled_tool_config_file(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path);
    matches!(
        base,
        "go.mod"
            | ".golangci.yml"
            | ".golangci.yaml"
            | "detekt.yml"
            | "detekt.yaml"
            | "build.gradle.kts"
            | "build.gradle"
            | "Directory.Build.props"
            | ".editorconfig"
    ) || base.ends_with(".csproj")
}
