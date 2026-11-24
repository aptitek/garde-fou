//! Pure `#![no_std]` types for the 3-tier ecosystem tool ingestion architecture (`GF080`–`GF082`).
//!
//! - **Tier 1 ([`LangRuleFinding`])**: language-specific AST idioms & security rules (`GF080`).
//! - **Tier 2 ([`ToolConfigFinding`])**: ecosystem tool config hardening (`GF081`).
//! - **Tier 3 ([`ExternalToolFinding`])**: normalized diagnostics ingested from external
//!   type-checkers and linters (`ruff`, `mypy`, `pyright`, `tsc`, `eslint`, `shellcheck`, `GF082`).

use alloc::format;
use alloc::string::String;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;

/// Identifier of a first-class ecosystem linter or type-checker (`GF081` / `GF082`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ExternalToolId {
    /// Astral `ruff` Python linter (`ruff check --output-format json`).
    Ruff,
    /// `mypy` static type checker (`mypy --strict --output json`).
    Mypy,
    /// `pyright` static type checker (`pyright --outputjson`).
    Pyright,
    /// TypeScript compiler (`tsc --noEmit`).
    Tsc,
    /// `eslint` JavaScript/TypeScript linter.
    Eslint,
    /// `shellcheck` POSIX shell script analyzer.
    Shellcheck,
    /// `cargo-mutants` Rust mutation testing runner (`mutants.toml`).
    CargoMutants,
    /// `Stryker` JS/TS mutation testing runner (`stryker.config.json`).
    Stryker,
}

impl ExternalToolId {
    /// All known ecosystem tool identifiers in deterministic order.
    pub const ALL: [Self; 8] = [
        Self::Ruff,
        Self::Mypy,
        Self::Pyright,
        Self::Tsc,
        Self::Eslint,
        Self::Shellcheck,
        Self::CargoMutants,
        Self::Stryker,
    ];

    /// Canonical lowercase CLI/config name of the tool.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ruff => "ruff",
            Self::Mypy => "mypy",
            Self::Pyright => "pyright",
            Self::Tsc => "tsc",
            Self::Eslint => "eslint",
            Self::Shellcheck => "shellcheck",
            Self::CargoMutants => "cargo-mutants",
            Self::Stryker => "stryker",
        }
    }
}

impl fmt::Display for ExternalToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tier 1 native AST language rule violation (`GF080`), requiring zero external binaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LangRuleFinding {
    /// Upstream ecosystem rule code (e.g. `"PY-B006"`, `"PY-S602"`, `"PY-ANN401"`).
    pub code: String,
    /// 1-based source line span of the offending AST construct.
    pub span: Span,
    /// Concrete description of the anti-pattern or security hazard.
    pub message: String,
    /// Actionable root-cause remediation in the target language.
    pub remediation: String,
}

/// Tier 2 static configuration weakening finding (`GF081`) in `pyproject.toml`,
/// `ruff.toml`, `.ruff.toml`, or `tsconfig.json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolConfigFinding {
    /// Ecosystem tool whose configuration is weakened.
    pub tool: ExternalToolId,
    /// Dotted configuration key path (e.g. `"tool.mypy.strict"`, `"compilerOptions.strict"`).
    pub key: String,
    /// 1-based source line span inside the configuration file.
    pub span: Span,
    /// Human/agent explanation of what setting is missing or weakened.
    pub message: String,
    /// Exact TOML/JSON snippet required to restore strict enforcement.
    pub remediation: String,
}

/// Tier 3 finding parsed from JSON/NDJSON output of an external tool (`GF082`).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ExternalToolFinding {
    /// Source ecosystem tool that emitted the diagnostic.
    pub tool: ExternalToolId,
    /// Normalized repository-relative path of the offending file.
    pub path: RepoPath,
    /// 1-based line number (`>= 1`).
    pub line: u32,
    /// 1-based column offset (`>= 1`).
    pub column: u32,
    /// Native rule or error code (e.g. `"F401"`, `"arg-type"`, `"reportGeneralTypeIssues"`).
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

/// Top-level `[tools]` language tool adapter configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsConfig {
    /// Python linter and type-checker policies (`[tools.python]`).
    pub python: PythonToolsConfig,
}

impl ToolsConfig {
    /// Returns strict default `[tools]` policies.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            python: PythonToolsConfig::defaults(),
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("tl({})", self.python.canonical_fragment())
    }
}

/// Returns `true` if `path` is a language-tool configuration file audited by Tier 2 (`GF081`).
#[must_use]
pub fn is_audited_tool_config_file(path: &RepoPath) -> bool {
    matches!(
        path.file_name(),
        "pyproject.toml"
            | "ruff.toml"
            | ".ruff.toml"
            | "tsconfig.json"
            | "mutants.toml"
            | "stryker.config.json"
            | "stryker.conf.json"
    )
}
