//! Pure `#![no_std]` Tier 2 & Tier 3 configuration types for PHP (`PHPStan`, `Psalm`, `Composer`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::external_tool_kind::ExternalToolKind;
use crate::policies::ScannerPolicy;

/// Tier 2 PHP static-analysis and Composer configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PhpConfigIssue {
    /// `phpstan.neon` level below 8 or blanket `ignoreErrors`.
    PhpstanLevelTooLowOrBlanketIgnore,
    /// `psalm.xml` `errorLevel` > 2 or blanket suppression.
    PsalmErrorLevelTooWeak,
    /// `composer.json` missing `require.php` or `autoload.psr-4`.
    ComposerMissingPhpOrPsr4,
}

impl PhpConfigIssue {
    /// Affected external tool.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::PhpstanLevelTooLowOrBlanketIgnore | Self::ComposerMissingPhpOrPsr4 => {
                ExternalToolKind::Phpstan
            }
            Self::PsalmErrorLevelTooWeak => ExternalToolKind::Psalm,
        }
    }

    /// Dotted configuration key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::PhpstanLevelTooLowOrBlanketIgnore => "parameters.level",
            Self::PsalmErrorLevelTooWeak => "psalm.errorLevel",
            Self::ComposerMissingPhpOrPsr4 => "autoload.psr-4",
        }
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::PhpstanLevelTooLowOrBlanketIgnore => {
                "Set `level: 8` (or `max`) in `phpstan.neon` and remove blanket `ignoreErrors` wildcards."
            }
            Self::PsalmErrorLevelTooWeak => "Tighten `errorLevel=\"1\"` or `\"2\"` in `psalm.xml`.",
            Self::ComposerMissingPhpOrPsr4 => {
                "Pin `require.php` (e.g. `>=8.4`) and declare `autoload.psr-4` in `composer.json`."
            }
        }
    }
}

/// `[tools.php]` execution policies for external PHP static analyzers (`GF082`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhpToolsConfig {
    /// Policy for `phpstan analyse --error-format=json` (`auto` by default).
    pub phpstan: ScannerPolicy,
    /// Policy for `psalm --output-format=json` (`off` by default).
    pub psalm: ScannerPolicy,
}

impl PhpToolsConfig {
    /// Default PHP tool policies (`phpstan = Auto`, `psalm = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            phpstan: ScannerPolicy::Auto,
            psalm: ScannerPolicy::Off,
        }
    }

    /// Deterministic cache-key fragment.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("php({},{})", self.phpstan.as_str(), self.psalm.as_str())
    }
}

impl Default for PhpToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` for PHP/Symfony Tier 2 audited config files (`composer.json`, `phpstan.neon`, `psalm.xml`).
#[must_use]
pub fn is_php_tool_config_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "composer.json" | "phpstan.neon" | "phpstan.neon.dist" | "psalm.xml" | "psalm.xml.dist"
    )
}
