//! Source [`Language`] classification, `tsconfig*.json` inheritance/alias metadata,
//! and TypeScript declaration file predicates.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Closed sum type identifying a first-class ecosystem linter, type-checker, or runner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ExternalToolKind {
    /// Astral `ruff` Python linter (`ruff check --output-format json`).
    Ruff,
    /// `mypy` static type checker (`mypy --strict --output json`).
    Mypy,
    /// `pyright` static type checker (`pyright --outputjson`).
    Pyright,
    /// `pytest` Python test runner (`[tool.pytest.ini_options]`).
    Pytest,
    /// `coverage.py` coverage gate (`[tool.coverage.report]`).
    Coverage,
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
    /// `oxlint` fast Rust-native JS/TS/React linter (`oxlint --format json`).
    Oxlint,
    /// `biome` fast Rust-native JS/TS/JSX analyzer (`biome check --reporter=json`).
    Biome,
    /// `hadolint` Dockerfile & embedded `ShellCheck` linter (`hadolint --format json`).
    Hadolint,
    /// `PSScriptAnalyzer` PowerShell AST static analyzer (`Invoke-ScriptAnalyzer`).
    PsScriptAnalyzer,
}

const TOOL_NAMES: [&str; 14] = [
    "ruff",
    "mypy",
    "pyright",
    "pytest",
    "coverage",
    "tsc",
    "eslint",
    "shellcheck",
    "cargo-mutants",
    "stryker",
    "oxlint",
    "biome",
    "hadolint",
    "psscriptanalyzer",
];

impl ExternalToolKind {
    /// All known ecosystem tool identifiers in deterministic order.
    pub const ALL: [Self; 14] = [
        Self::Ruff,
        Self::Mypy,
        Self::Pyright,
        Self::Pytest,
        Self::Coverage,
        Self::Tsc,
        Self::Eslint,
        Self::Shellcheck,
        Self::CargoMutants,
        Self::Stryker,
        Self::Oxlint,
        Self::Biome,
        Self::Hadolint,
        Self::PsScriptAnalyzer,
    ];

    /// Canonical lowercase CLI/config name of the tool.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        TOOL_NAMES[self as usize]
    }
}

impl fmt::Display for ExternalToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Source language, stylesheet, content, or asset format mapped into the `UAST`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    /// Rust source file (`.rs`).
    Rust,
    /// Python source file (`.py`).
    Python,
    /// TypeScript source file (`.ts`, `.mts`, `.cts`).
    TypeScript,
    /// TypeScript JSX source file (`.tsx`).
    Tsx,
    /// JavaScript source file (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// JavaScript JSX source file (`.jsx`).
    Jsx,
    /// Go source file (`.go`).
    Go,
    /// Container build recipe (`Dockerfile`, `Containerfile`).
    Dockerfile,
    /// Stylesheet in `Forme` (`.css`, `.scss`).
    Css,
    /// Editorial or governance Markdown/MDX (`.md`, `.mdx`).
    Markdown,
    /// JSON / JSONC document or `i18n` dictionary (`.json`, `.jsonc`).
    Json,
    /// TOML configuration or `i18n` dictionary (`.toml`).
    Toml,
    /// POSIX/Bash Shell script (`.sh`, `.bash`, or extensionless `sh`/`bash` shebang).
    Shell,
    /// PowerShell automation script or module (`.ps1`, `.psm1`, `.psd1`).
    PowerShell,
    /// Scalable Vector Graphics XML asset (`.svg`).
    Svg,
    /// Non-text binary media or font asset (`.png`, `.woff2`, `.pdf`, etc.).
    BinaryAsset,
    /// Unclassified text or source file.
    Unknown,
}

impl Language {
    /// Returns `true` if `gf-parsers` provides a programming-language / script / Dockerfile AST extractor.
    #[must_use]
    pub const fn is_code_analyzed(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::Python
                | Self::TypeScript
                | Self::Tsx
                | Self::JavaScript
                | Self::Jsx
                | Self::Shell
                | Self::PowerShell
                | Self::Dockerfile
        )
    }

    /// Returns `true` for all file formats analyzed directly across the 6 domains.
    #[must_use]
    pub const fn is_always_discovered(self) -> bool {
        self.is_code_analyzed()
            || matches!(
                self,
                Self::Css | Self::Markdown | Self::Svg | Self::BinaryAsset
            )
    }
}

/// Tri-state representation of `compilerOptions.strict` inside a `tsconfig*.json` file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TsconfigStrictState {
    /// `"strict": true` explicitly declared.
    ExplicitTrue,
    /// `"strict": false` explicitly declared.
    ExplicitFalse,
    /// `"strict"` omitted (valid only when inherited via `"extends"`).
    Omitted,
}

/// Parsed `tsconfig*.json` summary enabling workspace `"extends"` verification (`GF081`)
/// and `compilerOptions.baseUrl` / `paths` import alias resolution (`GF071` & `GF072`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TsconfigSummary {
    /// Optional `"extends"` parent specifier (relative path or preset package).
    pub extends: Option<String>,
    /// Declared state of `compilerOptions.strict`.
    pub strict: TsconfigStrictState,
    /// Optional `compilerOptions.baseUrl`.
    pub base_url: Option<String>,
    /// Optional `compilerOptions.paths` alias table (`pattern -> [targets]`).
    pub paths: BTreeMap<String, Vec<String>>,
    /// 1-based span used if an inherited `tsconfig` chain fails to enable `"strict": true`.
    pub strict_span: Span,
}

/// Returns `true` if `spec` names a well-known external TypeScript preset with `"strict": true`.
#[must_use]
pub fn is_known_strict_preset(spec: &str) -> bool {
    let s = spec.trim();
    s.starts_with("@tsconfig/") || s.contains("tsconfig/strict") || s.contains("tsconfigs/strict")
}

/// Returns `true` for ambient TypeScript declaration files (`*.d.ts`, `*.d.mts`, `*.d.cts`).
#[must_use]
pub fn is_ts_declaration_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let Some((stem, ext)) = lower.rsplit_once('.') else {
        return false;
    };
    let ends_with_d = stem.rsplit_once('.').is_some_and(|(_, sub)| sub == "d");
    matches!(ext, "ts" | "mts" | "cts") && (ends_with_d || stem.contains(".d."))
}
