//! Pure `#![no_std]` types for CSS, XML/SVG, TOML, and Markdown Tier 2 config issues (`GF081`)
//! and `[tools.{css,markdown,toml,xml}]` external verifier policies (`GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::language_meta::ExternalToolKind;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;

/// Specific `.stylelintrc*` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum StylelintConfigIssue {
    /// `color-no-hex`, `declaration-no-important`, or `selector-max-id` set to `null`/`false`.
    CriticalRuleDisabled,
}

/// Specific `.markdownlint*` or `rumdl.toml` / `.rumdl.toml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MarkdownConfigIssue {
    /// Global `"default": false` disabling all baseline Markdown rules.
    DefaultDisabled,
    /// `MD001` (`heading-increment`) or `MD033` (`no-inline-html`) disabled.
    CriticalRuleDisabled,
}

/// Specific `taplo.toml` / `.taplo.toml` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TaploConfigIssue {
    /// Global `exclude = ["**/*.toml"]` or `["*.toml"]` silencing TOML checks.
    WildcardExcluded,
}

/// Specific `svgo.config.{js,mjs,cjs}` configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SvgoConfigIssue {
    /// `removeScripts: false` leaving `<script>` XSS vectors intact in SVGs.
    RemoveScriptsDisabled,
}

/// Unified Tier 2 configuration weakening across CSS, Markdown, TOML, and SVG tools (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FormatConfigIssue {
    /// `.stylelintrc*` disabling token/specificity rules.
    Stylelint(StylelintConfigIssue),
    /// `.markdownlint*` or `rumdl.toml` disabling `"default"`, `MD001`, or `MD033`.
    Markdown(MarkdownConfigIssue),
    /// `taplo.toml` excluding `**/*.toml`.
    Taplo(TaploConfigIssue),
    /// `svgo.config.*` setting `removeScripts: false`.
    Svgo(SvgoConfigIssue),
}

impl FormatConfigIssue {
    /// Ecosystem tool affected by this configuration issue.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::Stylelint(_) => ExternalToolKind::Stylelint,
            Self::Markdown(_) => ExternalToolKind::Rumdl,
            Self::Taplo(_) => ExternalToolKind::Taplo,
            Self::Svgo(_) => ExternalToolKind::Xmllint,
        }
    }

    /// Canonical dotted key inside the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Stylelint(StylelintConfigIssue::CriticalRuleDisabled) => "rules",
            Self::Markdown(MarkdownConfigIssue::DefaultDisabled) => "default",
            Self::Markdown(MarkdownConfigIssue::CriticalRuleDisabled) => "disable",
            Self::Taplo(TaploConfigIssue::WildcardExcluded) => "exclude",
            Self::Svgo(SvgoConfigIssue::RemoveScriptsDisabled) => "plugins.removeScripts",
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Stylelint(_) => {
                "Keep `color-no-hex`, `declaration-no-important`, and `selector-max-id` enabled in `.stylelintrc`."
            }
            Self::Markdown(_) => {
                "Keep `default: true` and keep `MD001` (`heading-increment`) and `MD033` (`no-inline-html`) enabled in `.markdownlint` / `rumdl.toml`."
            }
            Self::Taplo(_) => {
                "Remove blanket `**/*.toml` / `*` patterns from `exclude` in `taplo.toml`."
            }
            Self::Svgo(_) => "Never set `removeScripts: false` in `svgo.config.js`.",
        }
    }
}

/// `[tools.css]` execution policy (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CssToolsConfig {
    /// Policy for `stylelint --formatter json` (`auto` by default).
    #[serde(default)]
    pub stylelint: ScannerPolicy,
}

impl CssToolsConfig {
    /// Default `[tools.css]` policy (`stylelint = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            stylelint: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("css(sl={})", self.stylelint.as_str())
    }
}

impl Default for CssToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.markdown]` execution policies (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkdownToolsConfig {
    /// Policy for `rumdl check --output-format json` (`auto` by default).
    #[serde(default)]
    pub rumdl: ScannerPolicy,
    /// Policy for `markdownlint-cli2` / `markdownlint --json` (`off` by default).
    #[serde(default)]
    pub markdownlint: ScannerPolicy,
}

impl MarkdownToolsConfig {
    /// Default `[tools.markdown]` policies (`rumdl = Auto`, `markdownlint = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            rumdl: ScannerPolicy::Auto,
            markdownlint: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "md(rm={},ml={})",
            self.rumdl.as_str(),
            self.markdownlint.as_str()
        )
    }
}

impl Default for MarkdownToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.toml]` execution policy (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlToolsConfig {
    /// Policy for `taplo check` (`auto` by default).
    #[serde(default)]
    pub taplo: ScannerPolicy,
}

impl TomlToolsConfig {
    /// Default `[tools.toml]` policy (`taplo = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            taplo: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("tm(tp={})", self.taplo.as_str())
    }
}

impl Default for TomlToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.xml]` execution policy (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmlToolsConfig {
    /// Policy for `xmllint --noout --nonet` (`off` by default unless configured).
    #[serde(default)]
    pub xmllint: ScannerPolicy,
}

impl XmlToolsConfig {
    /// Default `[tools.xml]` policy (`xmllint = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            xmllint: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("xml(xl={})", self.xmllint.as_str())
    }
}

impl Default for XmlToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` if `path` is a CSS, Markdown, TOML, or SVGO linter config audited by `GF081`.
#[must_use]
pub fn is_format_tool_config_file(path: &RepoPath) -> bool {
    const CONFIG_FILES: &[&str] = &[
        ".stylelintrc",
        ".stylelintrc.json",
        ".stylelintrc.yml",
        ".stylelintrc.yaml",
        ".markdownlint.json",
        ".markdownlint.jsonc",
        ".markdownlint.yml",
        ".markdownlint.yaml",
        "rumdl.toml",
        ".rumdl.toml",
        "taplo.toml",
        ".taplo.toml",
        "svgo.config.js",
        "svgo.config.mjs",
        "svgo.config.cjs",
    ];
    CONFIG_FILES.contains(&path.file_name().to_ascii_lowercase().as_str())
}
