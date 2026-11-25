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
    /// `yamllint` YAML 1.2 linter (`yamllint -f parsable`).
    Yamllint,
    /// `dclint` Docker Compose linter (`dclint -f json`).
    Dclint,
    /// `actionlint` GitHub Actions workflow verifier (`actionlint -format '{{json .}}'`).
    Actionlint,
    /// `zizmor` GitHub Actions security scanner (`zizmor --format sarif`).
    Zizmor,
    /// `stylelint` W3C CSS & Design Token linter (`stylelint --formatter json`).
    Stylelint,
    /// `rumdl` Rust-native `CommonMark`/GFM linter (`rumdl check --output-format json`).
    Rumdl,
    /// `markdownlint-cli2` / `markdownlint` Markdown linter (`--json`).
    Markdownlint,
    /// `taplo` TOML 1.0/1.1 & JSON-Schema verifier (`taplo check`).
    Taplo,
    /// `xmllint` XML 1.0 well-formedness & DTD verifier (`xmllint --noout --nonet`).
    Xmllint,
    /// `squawk` Rust-native PostgreSQL zero-downtime migration linter (`squawk --reporter Json`).
    Squawk,
    /// `sqruff` Rust-native multi-dialect SQL linter (`sqruff lint -f json`).
    Sqruff,
    /// `sqlfluff` Python multi-dialect SQL linter (`sqlfluff lint --format json`).
    Sqlfluff,
}

const TOOL_NAMES: [&str; 26] = [
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
    "yamllint",
    "dclint",
    "actionlint",
    "zizmor",
    "stylelint",
    "rumdl",
    "markdownlint",
    "taplo",
    "xmllint",
    "squawk",
    "sqruff",
    "sqlfluff",
];

impl ExternalToolKind {
    /// All known ecosystem tool identifiers in deterministic order.
    pub const ALL: [Self; 26] = [
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
        Self::Yamllint,
        Self::Dclint,
        Self::Actionlint,
        Self::Zizmor,
        Self::Stylelint,
        Self::Rumdl,
        Self::Markdownlint,
        Self::Taplo,
        Self::Xmllint,
        Self::Squawk,
        Self::Sqruff,
        Self::Sqlfluff,
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
    /// YAML 1.2 document, Docker Compose spec, or GitHub Actions workflow (`.yml`, `.yaml`).
    Yaml,
    /// POSIX/Bash Shell script (`.sh`, `.bash`, or extensionless `sh`/`bash` shebang).
    Shell,
    /// PowerShell automation script or module (`.ps1`, `.psm1`, `.psd1`).
    PowerShell,
    /// Scalable Vector Graphics XML asset (`.svg`).
    Svg,
    /// XML document or package manifest (`.xml`, `.csproj`, `.props`, `pom.xml`).
    Xml,
    /// SQL query, view, or database migration script (`.sql`).
    Sql,
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
                Self::Yaml
                    | Self::Css
                    | Self::Markdown
                    | Self::Toml
                    | Self::Xml
                    | Self::Svg
                    | Self::Sql
                    | Self::BinaryAsset
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

pub use crate::ts_violations::{is_known_strict_preset, is_ts_declaration_path};
