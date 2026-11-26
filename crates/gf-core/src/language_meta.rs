//! Source [`Language`] classification, `tsconfig*.json` inheritance/alias metadata,
//! and TypeScript declaration file predicates.

use serde::{Deserialize, Serialize};

pub use crate::external_tool_kind::ExternalToolKind;
pub use crate::ts_violations::{
    TsconfigStrictState, TsconfigSummary, is_known_strict_preset, is_ts_declaration_path,
};

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
    /// Kotlin source or script file (`.kt`, `.kts`).
    Kotlin,
    /// C# source file (`.cs`).
    CSharp,
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
    /// Oracle PL/SQL unit (`.pls`, `.plb`, `.pks`, `.pkb`, `.prc`, `.fnc`, `.trg`, `.tps`, `.tpb`).
    PlSql,
    /// Astro component / page / layout template (`.astro`).
    Astro,
    /// W3C HTML5 document (`.html`, `.htm`).
    Html,
    /// Jupyter Notebook JSON document (`.ipynb`, nbformat v4).
    Notebook,
    /// Quarto academic Markdown document (`.qmd`).
    Quarto,
    /// Obsidian JSON Canvas document (`.canvas`).
    JsonCanvas,
    /// Mermaid v11 diagram source (`.mmd`, `.mermaid`).
    Mermaid,
    /// `PlantUML` diagram source (`.puml`, `.plantuml`, `.iuml`, `.pu`).
    PlantUml,
    /// GNU Gettext translation catalog (`.po`, `.pot`).
    GettextPo,
    /// Project Fluent translation catalog (`.ftl`).
    FluentFtl,
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
                | Self::Go
                | Self::Kotlin
                | Self::CSharp
                | Self::Astro
                | Self::Shell
                | Self::PowerShell
                | Self::Dockerfile
                | Self::PlSql
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
                    | Self::Html
                    | Self::Notebook
                    | Self::Quarto
                    | Self::JsonCanvas
                    | Self::Mermaid
                    | Self::PlantUml
                    | Self::GettextPo
                    | Self::FluentFtl
                    | Self::BinaryAsset
            )
    }
}
