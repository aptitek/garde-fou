//! Unified `[tools]` configuration aggregating all language and format verifier policies.

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::format_tools::{CssToolsConfig, MarkdownToolsConfig, TomlToolsConfig, XmlToolsConfig};
use crate::shell_ps_tools::{DockerToolsConfig, PowerShellToolsConfig, ShellToolsConfig};
use crate::tool_ingest::{PythonToolsConfig, TypeScriptToolsConfig};
use crate::yaml_tools::YamlToolsConfig;

/// Top-level `[tools]` language and format tool adapter configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsConfig {
    /// Python linter and type-checker policies (`[tools.python]`).
    #[serde(default)]
    pub python: PythonToolsConfig,
    /// TypeScript/React linter and type-checker policies (`[tools.typescript]`).
    #[serde(default)]
    pub typescript: TypeScriptToolsConfig,
    /// POSIX/Bash `ShellCheck` policy (`[tools.shell]`).
    #[serde(default)]
    pub shell: ShellToolsConfig,
    /// PowerShell `PSScriptAnalyzer` policy (`[tools.powershell]`).
    #[serde(default)]
    pub powershell: PowerShellToolsConfig,
    /// Docker `hadolint` policy (`[tools.docker]`).
    #[serde(default)]
    pub docker: DockerToolsConfig,
    /// YAML, Docker Compose, and GitHub Actions policies (`[tools.yaml]`).
    #[serde(default)]
    pub yaml: YamlToolsConfig,
    /// W3C CSS `stylelint` policy (`[tools.css]`).
    #[serde(default)]
    pub css: CssToolsConfig,
    /// `CommonMark`/GFM `rumdl` and `markdownlint` policies (`[tools.markdown]`).
    #[serde(default)]
    pub markdown: MarkdownToolsConfig,
    /// TOML `taplo` policy (`[tools.toml]`).
    #[serde(default)]
    pub toml: TomlToolsConfig,
    /// XML/SVG `xmllint` policy (`[tools.xml]`).
    #[serde(default)]
    pub xml: XmlToolsConfig,
}

impl ToolsConfig {
    /// Returns strict default `[tools]` policies.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            python: PythonToolsConfig::defaults(),
            typescript: TypeScriptToolsConfig::defaults(),
            shell: ShellToolsConfig::defaults(),
            powershell: PowerShellToolsConfig::defaults(),
            docker: DockerToolsConfig::defaults(),
            yaml: YamlToolsConfig::defaults(),
            css: CssToolsConfig::defaults(),
            markdown: MarkdownToolsConfig::defaults(),
            toml: TomlToolsConfig::defaults(),
            xml: XmlToolsConfig::defaults(),
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "tl({},{},{},{},{},{},{},{},{},{})",
            self.python.canonical_fragment(),
            self.typescript.canonical_fragment(),
            self.shell.canonical_fragment(),
            self.powershell.canonical_fragment(),
            self.docker.canonical_fragment(),
            self.yaml.canonical_fragment(),
            self.css.canonical_fragment(),
            self.markdown.canonical_fragment(),
            self.toml.canonical_fragment(),
            self.xml.canonical_fragment(),
        )
    }
}
