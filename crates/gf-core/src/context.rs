//! Orthogonal project context model (`languages` × `features` × `frameworks`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

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
}

const LANG_NAMES: [&str; 10] = [
    "rust",
    "typescript",
    "python",
    "shell",
    "powershell",
    "yaml",
    "css",
    "xml",
    "toml",
    "markdown",
];
const LANGUAGE_ALIASES: &[(&str, LanguageId)] = &[
    ("rust", LanguageId::Rust),
    ("typescript", LanguageId::TypeScript),
    ("javascript", LanguageId::TypeScript),
    ("ts", LanguageId::TypeScript),
    ("js", LanguageId::TypeScript),
    ("python", LanguageId::Python),
    ("py", LanguageId::Python),
    ("shell", LanguageId::Shell),
    ("sh", LanguageId::Shell),
    ("bash", LanguageId::Shell),
    ("powershell", LanguageId::PowerShell),
    ("pwsh", LanguageId::PowerShell),
    ("ps1", LanguageId::PowerShell),
    ("yaml", LanguageId::Yaml),
    ("yml", LanguageId::Yaml),
    ("css", LanguageId::Css),
    ("scss", LanguageId::Css),
    ("xml", LanguageId::Xml),
    ("svg", LanguageId::Xml),
    ("toml", LanguageId::Toml),
    ("markdown", LanguageId::Markdown),
    ("md", LanguageId::Markdown),
    ("mdx", LanguageId::Markdown),
];

impl LanguageId {
    /// Parses a case-insensitive language identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        lookup_alias(LANGUAGE_ALIASES, raw)
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        LANG_NAMES[self as usize]
    }

    /// Returns `true` for primary code/automation languages (`Rust`, `TypeScript`, `Python`, `Shell`, `PowerShell`).
    #[must_use]
    pub const fn is_programming_language(self) -> bool {
        matches!(
            self,
            Self::Rust | Self::TypeScript | Self::Python | Self::Shell | Self::PowerShell
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
}

const FEATURE_NAMES: [&str; 9] = [
    "docker",
    "git-hooks",
    "ci-github",
    "task-runner",
    "ui",
    "i18n",
    "content",
    "tdd",
    "storybook",
];

const FEATURE_ALIASES: &[(&str, FeatureId)] = &[
    ("docker", FeatureId::Docker),
    ("container", FeatureId::Docker),
    ("git-hooks", FeatureId::GitHooks),
    ("githooks", FeatureId::GitHooks),
    ("ci-github", FeatureId::CiGithub),
    ("github-actions", FeatureId::CiGithub),
    ("task-runner", FeatureId::TaskRunner),
    ("taskrunner", FeatureId::TaskRunner),
    ("ui", FeatureId::Ui),
    ("tokens", FeatureId::Ui),
    ("a11y", FeatureId::Ui),
    ("i18n", FeatureId::I18n),
    ("locales", FeatureId::I18n),
    ("texte", FeatureId::I18n),
    ("content", FeatureId::Content),
    ("contenus", FeatureId::Content),
    ("markdown", FeatureId::Content),
    ("tdd", FeatureId::Tdd),
    ("storybook", FeatureId::Storybook),
    ("stories", FeatureId::Storybook),
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
    /// Active programming languages.
    pub languages: BTreeSet<LanguageId>,
    /// Active cross-cutting infrastructure features.
    pub features: BTreeSet<FeatureId>,
    /// Active application or workspace frameworks.
    pub frameworks: BTreeSet<FrameworkId>,
    /// Normalized entries of root `.dockerignore` (`None` when `.dockerignore` is absent).
    pub dockerignore_entries: Option<BTreeSet<String>>,
}

impl ProjectContext {
    /// Formats a deterministic one-line summary (`langs=[...] features=[...] frameworks=[...]`).
    #[must_use]
    pub fn summary_inline(&self) -> String {
        let langs: Vec<&str> = self.languages.iter().map(|l| l.as_str()).collect();
        let feats: Vec<&str> = self.features.iter().map(|f| f.as_str()).collect();
        let fws: Vec<&str> = self.frameworks.iter().map(|fw| fw.as_str()).collect();
        format!(
            "langs=[{}] features=[{}] frameworks=[{}]",
            langs.join(","),
            feats.join(","),
            fws.join(",")
        )
    }
}
