//! Orthogonal project context model (`languages` × `features` × `frameworks`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

pub use crate::archetype_policy::{ArchetypeSetting, WorkspaceArchetype};
pub use crate::framework_id::FrameworkId;

fn lookup_alias<T: Copy>(table: &[(&str, T)], raw: &str) -> Option<T> {
    table
        .iter()
        .find(|(alias, _)| raw.eq_ignore_ascii_case(alias))
        .map(|(_, id)| *id)
}

/// Primary programming and structured format languages active in a project.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LanguageId {
    /// Rust ecosystem (`.rs`, `Cargo.toml`).
    Rust,
    /// TypeScript / JavaScript ecosystem (`.ts`, `.tsx`, `.js`, `.jsx`).
    TypeScript,
    /// Python ecosystem (`.py`, `pyproject.toml`).
    Python,
    /// POSIX Shell / Bash ecosystem (`.sh`, `.bash`, extensionless `sh`/`bash` shebangs).
    Shell,
    /// PowerShell automation ecosystem (`.ps1`, `.psm1`, `.psd1`).
    PowerShell,
    /// YAML 1.2, Docker Compose, and GitHub Actions workflow ecosystem (`.yml`, `.yaml`).
    Yaml,
    /// W3C CSS & Design Tokens ecosystem (`.css`, `.scss`).
    Css,
    /// XML, SVG security, and XML manifest ecosystem (`.xml`, `.svg`, `.csproj`, `pom.xml`).
    Xml,
    /// TOML 1.0/1.1 & manifest ecosystem (`.toml`).
    Toml,
    /// `CommonMark`, GFM, and MDX editorial ecosystem (`.md`, `.mdx`).
    Markdown,
    /// Multi-dialect SQL & zero-downtime database migrations ecosystem (`.sql`).
    Sql,
    /// Astro component and island template ecosystem (`.astro`).
    Astro,
    /// W3C HTML5 document ecosystem (`.html`, `.htm`).
    Html,
    /// Go ecosystem (`.go`, `go.mod`, `go.work`).
    Go,
    /// Kotlin / JVM ecosystem (`.kt`, `.kts`, `build.gradle.kts`).
    Kotlin,
    /// C# / .NET ecosystem (`.cs`, `*.csproj`, `*.sln`).
    CSharp,
    /// Oracle PL/SQL procedural database ecosystem (`.pls`, `.plb`, `.pks`, `.pkb`, `.prc`, `.fnc`, `.trg`, `.tps`, `.tpb`).
    PlSql,
    /// Mermaid v11 diagrams-as-code (`.mmd`, `.mermaid`).
    Mermaid,
    /// `PlantUML` diagrams-as-code (`.puml`, `.plantuml`, `.iuml`, `.pu`).
    PlantUml,
    /// PHP 8.4 ecosystem (`.php`, `composer.json`, `phpstan.neon`).
    Php,
    /// Twig 3 template ecosystem (`.twig`, `.html.twig`).
    Twig,
}

#[rustfmt::skip]
const LANG_NAMES: [&str; 21] = [
    "rust", "typescript", "python", "shell", "powershell", "yaml", "css",
    "xml", "toml", "markdown", "sql", "astro", "html", "go", "kotlin", "csharp", "plsql",
    "mermaid", "plantuml", "php", "twig",
];
#[rustfmt::skip]
const LANGUAGE_ALIASES: &[(&str, LanguageId)] = &[
    ("rust", LanguageId::Rust), ("rs", LanguageId::Rust),
    ("typescript", LanguageId::TypeScript), ("javascript", LanguageId::TypeScript),
    ("ts", LanguageId::TypeScript), ("js", LanguageId::TypeScript),
    ("python", LanguageId::Python), ("py", LanguageId::Python),
    ("shell", LanguageId::Shell), ("sh", LanguageId::Shell), ("bash", LanguageId::Shell),
    ("powershell", LanguageId::PowerShell), ("pwsh", LanguageId::PowerShell), ("ps1", LanguageId::PowerShell),
    ("yaml", LanguageId::Yaml), ("yml", LanguageId::Yaml),
    ("css", LanguageId::Css), ("scss", LanguageId::Css),
    ("xml", LanguageId::Xml), ("svg", LanguageId::Xml),
    ("toml", LanguageId::Toml),
    ("markdown", LanguageId::Markdown), ("md", LanguageId::Markdown), ("mdx", LanguageId::Markdown),
    ("sql", LanguageId::Sql), ("postgres", LanguageId::Sql), ("sqlite", LanguageId::Sql), ("mariadb", LanguageId::Sql),
    ("astro", LanguageId::Astro), ("html", LanguageId::Html), ("htm", LanguageId::Html),
    ("go", LanguageId::Go), ("golang", LanguageId::Go),
    ("kotlin", LanguageId::Kotlin), ("kt", LanguageId::Kotlin), ("kts", LanguageId::Kotlin),
    ("csharp", LanguageId::CSharp), ("c#", LanguageId::CSharp), ("cs", LanguageId::CSharp), ("dotnet", LanguageId::CSharp),
    ("plsql", LanguageId::PlSql), ("oracle", LanguageId::PlSql), ("pl-sql", LanguageId::PlSql),
    ("mermaid", LanguageId::Mermaid), ("mmd", LanguageId::Mermaid),
    ("plantuml", LanguageId::PlantUml), ("puml", LanguageId::PlantUml),
    ("php", LanguageId::Php), ("twig", LanguageId::Twig),
];

impl LanguageId {
    /// Parses a case-insensitive language slug (`rust`/`rs`, `shell`/`sh`/`bash`, `powershell`/`ps1`/`pwsh`, etc.).
    #[must_use]
    pub fn from_slug(raw: &str) -> Option<Self> {
        lookup_alias(LANGUAGE_ALIASES, raw.trim())
    }

    /// Parses a case-insensitive language identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        Self::from_slug(raw)
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        LANG_NAMES[self as usize]
    }

    /// Returns `true` for primary code/automation languages (`Rust`, `TypeScript`, `Python`, `Go`, `Kotlin`, `CSharp`, `Shell`, `PowerShell`, `Astro`).
    #[must_use]
    pub const fn is_programming_language(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::TypeScript
                | Self::Python
                | Self::Go
                | Self::Kotlin
                | Self::CSharp
                | Self::Php
                | Self::Shell
                | Self::PowerShell
                | Self::Astro
                | Self::PlSql
        )
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Cross-cutting infrastructure and workflow capabilities orthogonal to languages.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FeatureId {
    /// Containerization (`Dockerfile`, `Containerfile`, `compose.yaml`).
    Docker,
    /// Automated Git hooks (`lefthook.yml`, `.husky`).
    GitHooks,
    /// GitHub Actions CI workflows (`.github/workflows`).
    CiGithub,
    /// Declarative task runner (`Taskfile.yml`, `justfile`, `mise.toml`).
    TaskRunner,
    /// Design Tokens & static UI accessibility (`styles/`, `tokens/`, `GF060`/`GF061`/`GF066`).
    Ui,
    /// Internationalization `Texte` dictionaries (`locales/`, `GF062`).
    I18n,
    /// Editorial `Contenus` pure Markdown/MDX (`content/`, `courses/`, `docs/`, `GF063`).
    Content,
    /// Strict `1:1` companion unit-test enforcement (`GF076`).
    Tdd,
    /// Storybook `.stories.tsx` parity enforcement (`GF076`).
    Storybook,
    /// Panda CSS zero-runtime build-time token extraction (`panda.config.*`, `styled-system/`).
    PandaCss,
    /// Material Design 3 Expressive (`m3e`: tonal pairing, Spatial/Effects springs, 48dp targets).
    Material3Expressive,
    /// W3C HTML5 content models, WAI-ARIA 1.2 & WCAG 2.2 AA/AAA compliance.
    W3c,
    /// AI, Machine Learning, Deep Learning & LLM Agent governance (`ML-*`, `LLM-*`).
    AiMl,
    /// Diagrams-as-Code (`Mermaid` v11 & `PlantUML`).
    Diagrams,
}

#[rustfmt::skip]
const FEATURE_NAMES: [&str; 14] = [
    "docker", "git-hooks", "ci-github", "task-runner", "ui", "i18n",
    "content", "tdd", "storybook", "panda-css", "m3e", "w3c", "ai-ml", "diagrams",
];

#[rustfmt::skip]
const FEATURE_ALIASES: &[(&str, FeatureId)] = &[
    ("docker", FeatureId::Docker), ("container", FeatureId::Docker), ("git-hooks", FeatureId::GitHooks),
    ("githooks", FeatureId::GitHooks), ("ci-github", FeatureId::CiGithub), ("github-actions", FeatureId::CiGithub),
    ("task-runner", FeatureId::TaskRunner), ("taskrunner", FeatureId::TaskRunner), ("ui", FeatureId::Ui),
    ("tokens", FeatureId::Ui), ("a11y", FeatureId::Ui), ("i18n", FeatureId::I18n),
    ("locales", FeatureId::I18n), ("texte", FeatureId::I18n), ("content", FeatureId::Content),
    ("contenus", FeatureId::Content), ("markdown", FeatureId::Content), ("tdd", FeatureId::Tdd),
    ("storybook", FeatureId::Storybook), ("stories", FeatureId::Storybook),
    ("panda-css", FeatureId::PandaCss), ("pandacss", FeatureId::PandaCss), ("panda", FeatureId::PandaCss),
    ("m3e", FeatureId::Material3Expressive), ("material3-expressive", FeatureId::Material3Expressive),
    ("material-3-expressive", FeatureId::Material3Expressive), ("md3", FeatureId::Material3Expressive),
    ("w3c", FeatureId::W3c), ("wcag", FeatureId::W3c), ("wcag22", FeatureId::W3c), ("html5", FeatureId::W3c),
    ("ai-ml", FeatureId::AiMl), ("ai", FeatureId::AiMl), ("ml", FeatureId::AiMl), ("llm", FeatureId::AiMl),
    ("diagrams", FeatureId::Diagrams), ("mermaid", FeatureId::Diagrams), ("plantuml", FeatureId::Diagrams),
];

impl FeatureId {
    /// Parses a case-insensitive feature identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        lookup_alias(FEATURE_ALIASES, raw)
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        FEATURE_NAMES[self as usize]
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Complete resolved context of the analyzed workspace.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Resolved proportional workspace archetype (`Minimal` .. `Software`).
    #[serde(default)]
    pub archetype: WorkspaceArchetype,
    /// Active programming languages (may be empty for `Minimal` / `Knowledge` / `Infra`).
    pub languages: BTreeSet<LanguageId>,
    /// Active cross-cutting infrastructure features.
    pub features: BTreeSet<FeatureId>,
    /// Active application or workspace frameworks.
    pub frameworks: BTreeSet<FrameworkId>,
    /// Normalized entries of root `.dockerignore` (`None` when `.dockerignore` is absent).
    pub dockerignore_entries: Option<BTreeSet<String>>,
}

impl ProjectContext {
    /// Formats a deterministic one-line summary (`archetype=... langs=[...] features=[...] frameworks=[...]`).
    #[must_use]
    pub fn summary_inline(&self) -> String {
        let langs: Vec<&str> = self.languages.iter().map(|l| l.as_str()).collect();
        let feats: Vec<&str> = self.features.iter().map(|f| f.as_str()).collect();
        let fws: Vec<&str> = self.frameworks.iter().map(|fw| fw.as_str()).collect();
        format!(
            "archetype={} langs=[{}] features=[{}] frameworks=[{}]",
            self.archetype.as_str(),
            langs.join(","),
            feats.join(","),
            fws.join(",")
        )
    }
}
