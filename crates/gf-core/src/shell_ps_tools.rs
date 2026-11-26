//! Pure `#![no_std]` types for Shell, PowerShell, and Docker Tier 2 config issues (`GF081`)
//! and `[tools.shell]` / `[tools.powershell]` / `[tools.docker]` adapter policies (`GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;

/// Specific `.shellcheckrc` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ShellcheckConfigIssue {
    /// `disable=all` globally disabling all `ShellCheck` rules.
    DisableAll,
    /// Global range disable such as `disable=SC1000-SC9999`.
    GlobalRangeDisable,
}

impl ShellcheckConfigIssue {
    /// Canonical key inside `.shellcheckrc`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::DisableAll | Self::GlobalRangeDisable => "disable",
        }
    }
}

/// Specific `.hadolint.yaml` / `hadolint.yaml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum HadolintConfigIssue {
    /// `ignored:` silencing a critical security/correctness rule (`DL3002`, `DL3007`, `DL4006`, `SC2086`).
    CriticalRuleIgnored,
    /// `trustedRegistries: ["*"]` disabling registry trust enforcement.
    WildcardTrustedRegistry,
}

impl HadolintConfigIssue {
    /// Canonical key inside `.hadolint.yaml`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CriticalRuleIgnored => "ignored",
            Self::WildcardTrustedRegistry => "trustedRegistries",
        }
    }
}

/// Specific `PSScriptAnalyzerSettings.psd1` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PsScriptAnalyzerConfigIssue {
    /// `ExcludeRules = @('*')` disabling all PowerShell rules.
    WildcardExcludeRules,
    /// Excluding `PSAvoidUsingInvokeExpression` or `PSAvoidUsingPlainTextForPassword`.
    CriticalSecurityRuleExcluded,
}

impl PsScriptAnalyzerConfigIssue {
    /// Canonical key inside `PSScriptAnalyzerSettings.psd1`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::WildcardExcludeRules | Self::CriticalSecurityRuleExcluded => "ExcludeRules",
        }
    }
}

/// Unified Shell/Docker/PowerShell Tier 2 configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ShellInfraConfigIssue {
    /// `.shellcheckrc` disabling `all` or broad `SC1000-SC9999` ranges.
    Shellcheck(ShellcheckConfigIssue),
    /// `.hadolint.yaml` ignoring critical rules or wildcarding `trustedRegistries`.
    Hadolint(HadolintConfigIssue),
    /// `PSScriptAnalyzerSettings.psd1` excluding `*` or critical security rules.
    PsScriptAnalyzer(PsScriptAnalyzerConfigIssue),
}

impl ShellInfraConfigIssue {
    /// Ecosystem tool affected by this configuration issue.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::Shellcheck(_) => ExternalToolKind::Shellcheck,
            Self::Hadolint(_) => ExternalToolKind::Hadolint,
            Self::PsScriptAnalyzer(_) => ExternalToolKind::PsScriptAnalyzer,
        }
    }

    /// Canonical key inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Shellcheck(k) => k.key(),
            Self::Hadolint(k) => k.key(),
            Self::PsScriptAnalyzer(k) => k.key(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Shellcheck(_) => {
                "Remove `disable=all` and global `SCxxxx-SCyyyy` ranges from `.shellcheckrc`; fix ShellCheck findings at their source."
            }
            Self::Hadolint(_) => {
                "Do not silence critical rules (`DL3002`, `DL3007`, `DL4006`, `SC2086`) or allow `trustedRegistries: [\"*\"]` in `.hadolint.yaml`."
            }
            Self::PsScriptAnalyzer(_) => {
                "Never set `ExcludeRules = @('*')` or exclude `PSAvoidUsingInvokeExpression` / `PSAvoidUsingPlainTextForPassword` in `PSScriptAnalyzerSettings.psd1`."
            }
        }
    }
}

/// `[tools.shell]` execution policy for `shellcheck` (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellToolsConfig {
    /// Policy for `shellcheck --format=json1` (`auto` by default).
    pub shellcheck: ScannerPolicy,
}

impl ShellToolsConfig {
    /// Default Shell tool policy (`shellcheck = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            shellcheck: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("sh(sc={})", self.shellcheck.as_str())
    }
}

impl Default for ShellToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.powershell]` execution policy for `PSScriptAnalyzer` (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerShellToolsConfig {
    /// Policy for `Invoke-ScriptAnalyzer` via `pwsh` (`off` by default unless configured).
    pub psscriptanalyzer: ScannerPolicy,
}

impl PowerShellToolsConfig {
    /// Default PowerShell tool policy (`psscriptanalyzer = Off` to avoid slow .NET cold-start unless requested).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            psscriptanalyzer: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("ps(psa={})", self.psscriptanalyzer.as_str())
    }
}

impl Default for PowerShellToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.docker]` execution policy for `hadolint` (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerToolsConfig {
    /// Policy for `hadolint --format json` (`auto` by default).
    pub hadolint: ScannerPolicy,
}

impl DockerToolsConfig {
    /// Default Docker tool policy (`hadolint = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            hadolint: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("dk(hl={})", self.hadolint.as_str())
    }
}

impl Default for DockerToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` if `file_name` is `tsconfig.json` or a profile variant (`tsconfig.*.json`).
#[must_use]
pub fn is_tsconfig_filename(file_name: &str) -> bool {
    file_name.eq_ignore_ascii_case("tsconfig.json")
        || (file_name.starts_with("tsconfig.")
            && file_name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("json")))
}

/// Returns `true` if `path` is a language-tool configuration file audited by Tier 2 (`GF081`).
#[must_use]
pub fn is_audited_tool_config_file(path: &RepoPath) -> bool {
    let name = path.file_name();
    is_tsconfig_filename(name)
        || crate::yaml_tools::is_yaml_tool_config_file(path)
        || crate::format_tools::is_format_tool_config_file(path)
        || crate::sql_tools::is_sql_tool_config_file(path)
        || crate::web_edge_summary::is_web_edge_tool_config_file(path)
        || crate::compiled_tools::is_compiled_tool_config_file(path.as_str())
        || crate::toolchain_mern_violations::is_toolchain_audited_file(path.as_str())
        || name.eq_ignore_ascii_case("psscriptanalyzersettings.psd1")
        || matches!(
            name,
            "pyproject.toml"
                | "ruff.toml"
                | ".ruff.toml"
                | "biome.json"
                | "biome.jsonc"
                | "mutants.toml"
                | "stryker.config.json"
                | "stryker.conf.json"
                | ".shellcheckrc"
                | "shellcheckrc"
                | ".hadolint.yaml"
                | ".hadolint.yml"
                | "hadolint.yaml"
                | "hadolint.yml"
        )
}
