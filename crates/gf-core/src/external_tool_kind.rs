//! Closed sum type [`ExternalToolKind`] identifying first-class ecosystem linters and verifiers.

use core::fmt;
use serde::{Deserialize, Serialize};

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
    /// `astro check` Astro component & TypeScript verifier (`astro check`).
    AstroCheck,
    /// `html-validate` W3C HTML5 & WCAG verifier (`html-validate --formatter json`).
    HtmlValidate,
    /// `wrangler types` Cloudflare Workers binding type verifier (`wrangler types --check`).
    WranglerTypes,
    /// `golangci-lint` v1/v2 Go meta-linter (`golangci-lint run`).
    GolangciLint,
    /// `go vet -json ./...` Go static analyzer.
    GoVet,
    /// `detekt` Kotlin static analyzer (`detekt --report sarif:...`).
    Detekt,
    /// `ktlint` Kotlin linter (`ktlint --reporter=json`).
    Ktlint,
    /// `dotnet build` C# / Roslyn `MSBuild` compiler & analyzers.
    DotnetBuild,
    /// `pnpm` exclusive JS/TS/Node package manager (`GF081`).
    Pnpm,
    /// `uv` exclusive Python package & environment manager (`GF081`).
    Uv,
    /// Gradle Kotlin DSL (`build.gradle.kts` + `libs.versions.toml`, `GF081`).
    Gradle,
    /// Z PL/SQL Analyzer (`zpa-cli`) Oracle PL/SQL static analyzer (`GF082`).
    Zpa,
}

#[rustfmt::skip]
const TOOL_NAMES: [&str; 38] = [
    "ruff", "mypy", "pyright", "pytest", "coverage", "tsc", "eslint", "shellcheck",
    "cargo-mutants", "stryker", "oxlint", "biome", "hadolint", "psscriptanalyzer",
    "yamllint", "dclint", "actionlint", "zizmor", "stylelint", "rumdl", "markdownlint",
    "taplo", "xmllint", "squawk", "sqruff", "sqlfluff", "astro-check", "html-validate",
    "wrangler-types", "golangci-lint", "go-vet", "detekt", "ktlint", "dotnet-build",
    "pnpm", "uv", "gradle", "zpa",
];

impl ExternalToolKind {
    /// All known ecosystem tool identifiers in deterministic order.
    #[rustfmt::skip]
    pub const ALL: [Self; 38] = [
        Self::Ruff, Self::Mypy, Self::Pyright, Self::Pytest, Self::Coverage, Self::Tsc,
        Self::Eslint, Self::Shellcheck, Self::CargoMutants, Self::Stryker, Self::Oxlint,
        Self::Biome, Self::Hadolint, Self::PsScriptAnalyzer, Self::Yamllint, Self::Dclint,
        Self::Actionlint, Self::Zizmor, Self::Stylelint, Self::Rumdl, Self::Markdownlint,
        Self::Taplo, Self::Xmllint, Self::Squawk, Self::Sqruff, Self::Sqlfluff,
        Self::AstroCheck, Self::HtmlValidate, Self::WranglerTypes, Self::GolangciLint,
        Self::GoVet, Self::Detekt, Self::Ktlint, Self::DotnetBuild, Self::Pnpm, Self::Uv,
        Self::Gradle, Self::Zpa,
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
