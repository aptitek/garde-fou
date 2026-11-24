//! Pure `#![no_std]` types for the 3-tier ecosystem tool ingestion architecture (`GF080`–`GF082`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
use crate::path::RepoPath;
use crate::policies::ScannerPolicy;
pub use crate::python_violations::ExternalToolKind;
use crate::python_violations::PythonViolation;
use crate::tool_config_violations::ToolConfigViolation;

/// Type alias preserving compatibility with [`ExternalToolKind`].
pub type ExternalToolId = ExternalToolKind;

/// Tier 1 native AST language rule violation (`GF080`), requiring zero external binaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LangRuleFinding {
    /// Closed algebraic Python AST violation category (`PY-B006`, `PY-ANN201`, etc.).
    pub violation: PythonViolation,
    /// 1-based source line span of the offending AST construct.
    pub span: Span,
    /// Concrete description of the anti-pattern or security hazard.
    pub message: String,
}

impl LangRuleFinding {
    /// Canonical ecosystem rule code (`"PY-..."`).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.violation.code()
    }

    /// Actionable root-cause remediation derived from [`PythonViolation`].
    #[must_use]
    pub const fn remediation(&self) -> &'static str {
        self.violation.remediation()
    }
}

/// Tier 2 static configuration weakening finding (`GF081`) in `pyproject.toml`,
/// `ruff.toml`, `.ruff.toml`, `tsconfig.json`, `mutants.toml`, or `stryker.config.json`.
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

/// Tier 3 finding parsed from JSON/NDJSON output of an external tool (`GF082`).
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
