//! Unified `[tools]` configuration aggregating all language and format verifier policies.

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::compiled_tools::{CSharpToolsConfig, GoToolsConfig, KotlinToolsConfig};
use crate::dart_tools::DartToolsConfig;
use crate::format_tools::{CssToolsConfig, MarkdownToolsConfig, TomlToolsConfig, XmlToolsConfig};
use crate::php_tools::PhpToolsConfig;
use crate::shell_ps_tools::{DockerToolsConfig, PowerShellToolsConfig, ShellToolsConfig};
use crate::sql_tools::SqlToolsConfig;
use crate::tool_ingest::{PythonToolsConfig, TypeScriptToolsConfig};
use crate::web_edge_tools::WebToolsConfig;
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
    /// SQL & migration `squawk`, `sqruff`, and `sqlfluff` policies (`[tools.sql]`).
    #[serde(default)]
    pub sql: SqlToolsConfig,
    /// Web & Edge `astro-check`, `html-validate`, and `wrangler-types` policies (`[tools.web]`).
    #[serde(default)]
    pub web: WebToolsConfig,
    /// Go `golangci-lint` and `go vet` policies (`[tools.go]`).
    #[serde(default)]
    pub go: GoToolsConfig,
    /// Kotlin `detekt` and `ktlint` policies (`[tools.kotlin]`).
    #[serde(default)]
    pub kotlin: KotlinToolsConfig,
    /// C# / .NET `dotnet build` policy (`[tools.csharp]`).
    #[serde(default)]
    pub csharp: CSharpToolsConfig,
    /// PHP `PHPStan` and `Psalm` policies (`[tools.php]`).
    #[serde(default)]
    pub php: PhpToolsConfig,
    /// Dart / Flutter `dart analyze` policy (`[tools.dart]`).
    #[serde(default)]
    pub dart: DartToolsConfig,
}

impl ToolsConfig {
    /// Strict default `[tools]` policies constant.
    pub const DEFAULT: Self = Self {
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
        sql: SqlToolsConfig::defaults(),
        web: WebToolsConfig::defaults(),
        go: GoToolsConfig::defaults(),
        kotlin: KotlinToolsConfig::defaults(),
        csharp: CSharpToolsConfig::defaults(),
        php: PhpToolsConfig::defaults(),
        dart: DartToolsConfig::defaults(),
    };

    /// Returns strict default `[tools]` policies.
    #[must_use]
    pub const fn defaults() -> Self {
        Self::DEFAULT
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "tl({},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{})",
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
            self.sql.canonical_fragment(),
            self.web.canonical_fragment(),
            self.go.canonical_fragment(),
            self.kotlin.canonical_fragment(),
            self.csharp.canonical_fragment(),
            self.php.canonical_fragment(),
            self.dart.canonical_fragment(),
        )
    }
}
