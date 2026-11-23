//! Pure `#![no_std]` policies for `[scanners]` (`gitleaks`, `osv-scanner`) and `[coverage]` (`LCOV`).

use alloc::string::String;
use serde::{Deserialize, Serialize};

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
