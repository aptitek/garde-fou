//! Pure `#![no_std]` types for the 3-tier ecosystem tool ingestion architecture (`GF080`–`GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
pub use crate::language_meta::ExternalToolKind;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;
use crate::tool_config_violations::ToolConfigViolation;
use crate::ts_violations::LangRuleViolation;

/// Type alias preserving compatibility with [`ExternalToolKind`].
pub type ExternalToolId = ExternalToolKind;

/// Tier 1 native AST language rule violation (`GF080`), requiring zero external binaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LangRuleFinding {
    /// Closed algebraic language AST violation (`Python`, `TypeScript`, or `React`).
    pub violation: LangRuleViolation,
    /// 1-based source line span of the offending AST construct.
    pub span: Span,
    /// Concrete description of the anti-pattern or security hazard.
    pub message: String,
}

impl LangRuleFinding {
    /// Constructs a [`LangRuleFinding`] from any language violation convertible into [`LangRuleViolation`].
    #[must_use]
    pub fn new(
        violation: impl Into<LangRuleViolation>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            violation: violation.into(),
            span,
            message: message.into(),
        }
    }

    /// Canonical ecosystem rule code (`"PY-..."`, `"TS-..."`, or `"REACT-..."`).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.violation.code()
    }

    /// Actionable root-cause remediation derived from [`LangRuleViolation`].
    #[must_use]
    pub const fn remediation(&self) -> &'static str {
        self.violation.remediation()
    }
}

/// Tier 2 static configuration weakening finding (`GF081`) in `pyproject.toml`,
/// `ruff.toml`, `.ruff.toml`, `tsconfig*.json`, `biome.json*`, `mutants.toml`, or `stryker.config.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolConfigFinding {
    /// Closed algebraic configuration weakening category.
    pub violation: ToolConfigViolation,
    /// 1-based source line span inside the configuration file.
    pub span: Span,
    /// Human/agent explanation of what setting is missing or weakened.
    pub message: String,
}

impl ToolConfigFinding {
    /// Ecosystem tool affected by this configuration weakening.
    #[must_use]
    pub const fn tool(&self) -> ExternalToolKind {
        self.violation.tool()
    }

    /// Canonical dotted key path inside the configuration file.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.violation.key()
    }

    /// Actionable root-cause remediation derived from [`ToolConfigViolation`].
    #[must_use]
    pub const fn remediation(&self) -> &'static str {
        self.violation.remediation()
    }
}

/// Tier 3 finding parsed from JSON/NDJSON/text output of an external tool (`GF082`).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ExternalToolFinding {
    /// Source ecosystem tool that emitted the diagnostic.
    pub tool: ExternalToolKind,
    /// Normalized repository-relative path of the offending file.
    pub path: RepoPath,
    /// 1-based line number (`>= 1`).
    pub line: u32,
    /// 1-based column offset (`>= 1`).
    pub column: u32,
    /// Native rule or error code (e.g. `"F401"`, `"TS2322"`, `"lint/suspicious/noExplicitAny"`).
    pub native_code: String,
    /// Primary diagnostic message reported by the tool.
    pub message: String,
    /// Concrete fix or hint extracted from the tool output (or structured guidance).
    pub remediation: String,
}

/// `[tools.python]` execution policies for external Python verifiers (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonToolsConfig {
    /// Policy for `ruff check` (`auto` by default).
    pub ruff: ScannerPolicy,
    /// Policy for `mypy --strict` (`auto` by default).
    pub mypy: ScannerPolicy,
    /// Policy for `pyright` (`off` by default).
    pub pyright: ScannerPolicy,
}

impl PythonToolsConfig {
    /// Default Python tool policies (`ruff = Auto`, `mypy = Auto`, `pyright = Off`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            ruff: ScannerPolicy::Auto,
            mypy: ScannerPolicy::Auto,
            pyright: ScannerPolicy::Off,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "py(r={},m={},p={})",
            self.ruff.as_str(),
            self.mypy.as_str(),
            self.pyright.as_str(),
        )
    }
}

impl Default for PythonToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// `[tools.typescript]` execution policies for external TypeScript/React verifiers (`GF082` / `GF043`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeScriptToolsConfig {
    /// Policy for `oxlint --format json` (`auto` by default).
    pub oxlint: ScannerPolicy,
    /// Policy for `biome check --reporter=json` (`off` by default).
    pub biome: ScannerPolicy,
    /// Policy for `tsc --noEmit --pretty false` (`auto` by default).
    pub tsc: ScannerPolicy,
}

impl TypeScriptToolsConfig {
    /// Default TypeScript tool policies (`oxlint = Auto`, `biome = Off`, `tsc = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            oxlint: ScannerPolicy::Auto,
            biome: ScannerPolicy::Off,
            tsc: ScannerPolicy::Auto,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "ts(o={},b={},t={})",
            self.oxlint.as_str(),
            self.biome.as_str(),
            self.tsc.as_str(),
        )
    }
}

impl Default for TypeScriptToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Top-level `[tools]` language tool adapter configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsConfig {
    /// Python linter and type-checker policies (`[tools.python]`).
    pub python: PythonToolsConfig,
    /// TypeScript/React linter and type-checker policies (`[tools.typescript]`).
    #[serde(default)]
    pub typescript: TypeScriptToolsConfig,
}

impl ToolsConfig {
    /// Returns strict default `[tools]` policies.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            python: PythonToolsConfig::defaults(),
            typescript: TypeScriptToolsConfig::defaults(),
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!(
            "tl({},{})",
            self.python.canonical_fragment(),
            self.typescript.canonical_fragment(),
        )
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
        )
}
