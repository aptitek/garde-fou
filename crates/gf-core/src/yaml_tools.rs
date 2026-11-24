//! Pure `#![no_std]` types for YAML, Docker Compose, and GitHub Actions Tier 2 config issues (`GF081`)
//! and `[tools.yaml]` external verifier policies (`GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;

/// Specific `.yamllint` / `.yamllint.yml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum YamllintConfigIssue {
    /// `extends: relaxed` weakening core YAML strictness.
    ExtendsRelaxed,
    /// `truthy: disable` or `truthy: false` allowing YAML 1.1 boolean ambiguity.
    TruthyDisabled,
    /// `key-duplicates: disable` or `key-duplicates: false` allowing silent overwrites.
    KeyDuplicatesDisabled,
}

impl YamllintConfigIssue {
    /// Canonical dotted key inside `.yamllint`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::ExtendsRelaxed => "extends",
            Self::TruthyDisabled => "rules.truthy",
            Self::KeyDuplicatesDisabled => "rules.key-duplicates",
        }
    }
}

/// Specific `.dclintrc*` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DclintConfigIssue {
    /// Critical Compose security rule set to `0` or `"off"` under `rules`.
    CriticalRuleDisabled,
    /// Wildcard `exclude` silencing Compose validation across the repository.
    WildcardExcluded,
}

impl DclintConfigIssue {
    /// Canonical key inside `.dclintrc`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CriticalRuleDisabled => "rules",
            Self::WildcardExcluded => "exclude",
        }
    }
}

/// Specific `zizmor.yml` / `.github/zizmor.yml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ZizmorConfigIssue {
    /// `disable: true` on `template-injection`, `unpinned-uses`, `excessive-permissions`, or `dangerous-triggers`.
    CriticalRuleDisabled,
}

impl ZizmorConfigIssue {
    /// Canonical key inside `zizmor.yml`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CriticalRuleDisabled => "rules",
        }
    }
}

/// Unified YAML / Compose / GitHub Actions Tier 2 configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum YamlCiConfigIssue {
    /// `.yamllint*` using `extends: relaxed` or disabling `truthy` / `key-duplicates`.
    Yamllint(YamllintConfigIssue),
    /// `.dclintrc*` disabling critical Compose security rules or wildcarding `exclude`.
    Dclint(DclintConfigIssue),
    /// `zizmor.yml` disabling critical GitHub Actions security audits.
    Zizmor(ZizmorConfigIssue),
}

impl YamlCiConfigIssue {
    /// Ecosystem tool affected by this configuration issue.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::Yamllint(_) => ExternalToolKind::Yamllint,
            Self::Dclint(_) => ExternalToolKind::Dclint,
            Self::Zizmor(_) => ExternalToolKind::Zizmor,
        }
    }

    /// Canonical dotted key inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Yamllint(k) => k.key(),
            Self::Dclint(k) => k.key(),
            Self::Zizmor(k) => k.key(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Yamllint(_) => {
                "Keep `extends: default` and keep `truthy` and `key-duplicates` enabled in `.yamllint`."
            }
            Self::Dclint(_) => {
                "Do not disable critical Compose security rules or wildcard `exclude` in `.dclintrc`."
            }
            Self::Zizmor(_) => {
                "Keep `template-injection`, `unpinned-uses`, `excessive-permissions`, and `dangerous-triggers` enabled in `zizmor.yml`."
            }
        }
    }
}

/// `[tools.yaml]` execution policies for external YAML, Compose, and CI verifiers (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlToolsConfig {
    /// Policy for `yamllint -f parsable` (`auto` by default).
    pub yamllint: ScannerPolicy,
    /// Policy for `dclint -f json` (`auto` by default).
    pub dclint: ScannerPolicy,
    /// Policy for `actionlint -format '{{json .}}'` (`auto` by default).
    pub actionlint: ScannerPolicy,
    /// Policy for `zizmor --format sarif` (`auto` by default).
    pub zizmor: ScannerPolicy,
}

impl YamlToolsConfig {
    /// Default YAML/Compose/CI tool policies (`auto` across `yamllint`, `dclint`, `actionlint`, `zizmor`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            yamllint: ScannerPolicy::Auto,
            dclint: ScannerPolicy::Auto,
            actionlint: ScannerPolicy::Auto,
            zizmor: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "yml(yl={},dc={},al={},zz={})",
            self.yamllint.as_str(),
            self.dclint.as_str(),
            self.actionlint.as_str(),
            self.zizmor.as_str()
        )
    }
}

impl Default for YamlToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` if `path` is a `.yamllint*`, `.dclintrc*`, or `zizmor.yml` config audited by `GF081`.
#[must_use]
pub fn is_yaml_tool_config_file(path: &RepoPath) -> bool {
    let lower = path.file_name().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        ".yamllint"
            | ".yamllint.yml"
            | ".yamllint.yaml"
            | ".dclintrc"
            | ".dclintrc.json"
            | ".dclintrc.yml"
            | ".dclintrc.yaml"
            | "zizmor.yml"
    )
}
