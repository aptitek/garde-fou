//! Orthogonal project context model (`languages` × `features` × `frameworks`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Primary programming languages active in a project.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LanguageId {
    /// Rust ecosystem (`.rs`, `Cargo.toml`).
    Rust,
    /// TypeScript / JavaScript ecosystem (`.ts`, `.tsx`, `.js`, `.jsx`).
    TypeScript,
    /// Python ecosystem (`.py`, `pyproject.toml`).
    Python,
}

impl LanguageId {
    /// Parses a case-insensitive language identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("rust") {
            Some(Self::Rust)
        } else if raw.eq_ignore_ascii_case("typescript")
            || raw.eq_ignore_ascii_case("javascript")
            || raw.eq_ignore_ascii_case("ts")
            || raw.eq_ignore_ascii_case("js")
        {
            Some(Self::TypeScript)
        } else if raw.eq_ignore_ascii_case("python") || raw.eq_ignore_ascii_case("py") {
            Some(Self::Python)
        } else {
            None
        }
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Python => "python",
        }
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
    /// Internationalization `Texte` dictionaries & strict `t(...)` JSX check (`locales/`, `GF062`).
    I18n,
    /// Editorial `Contenus` pure Markdown/MDX (`content/`, `courses/`, `docs/`, `GF063`).
    Content,
}

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
];

impl FeatureId {
    /// Parses a case-insensitive feature identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        FEATURE_ALIASES
            .iter()
            .find(|(alias, _)| raw.eq_ignore_ascii_case(alias))
            .map(|(_, id)| *id)
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::GitHooks => "git-hooks",
            Self::CiGithub => "ci-github",
            Self::TaskRunner => "task-runner",
            Self::Ui => "ui",
            Self::I18n => "i18n",
            Self::Content => "content",
        }
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Application or workspace frameworks informing implicit entrypoint routing.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FrameworkId {
    /// Multi-crate or single-crate Cargo workspace.
    CargoWorkspace,
    /// React component library or SPA.
    React,
    /// Astro content/SSR framework (`src/pages/**`).
    Astro,
    /// Next.js Pages/App Router framework (`src/pages/**`, `src/app/**`).
    NextJs,
    /// `FastAPI` Python ASGI service.
    FastApi,
    /// Django Python web framework.
    Django,
}

impl FrameworkId {
    /// Parses a case-insensitive framework identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("cargo-workspace") || raw.eq_ignore_ascii_case("cargo") {
            Some(Self::CargoWorkspace)
        } else if raw.eq_ignore_ascii_case("react") {
            Some(Self::React)
        } else if raw.eq_ignore_ascii_case("astro") {
            Some(Self::Astro)
        } else if raw.eq_ignore_ascii_case("nextjs") || raw.eq_ignore_ascii_case("next") {
            Some(Self::NextJs)
        } else if raw.eq_ignore_ascii_case("fastapi") {
            Some(Self::FastApi)
        } else if raw.eq_ignore_ascii_case("django") {
            Some(Self::Django)
        } else {
            None
        }
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoWorkspace => "cargo-workspace",
            Self::React => "react",
            Self::Astro => "astro",
            Self::NextJs => "nextjs",
            Self::FastApi => "fastapi",
            Self::Django => "django",
        }
    }

    /// Returns implicit entrypoint path globs associated with this framework.
    #[must_use]
    pub const fn implicit_entrypoints(self) -> &'static [&'static str] {
        match self {
            Self::CargoWorkspace => &[
                "src/lib.rs",
                "src/main.rs",
                "crates/*/src/lib.rs",
                "crates/*/src/main.rs",
                "crates/*/tests/**",
                "tests/**",
            ],
            Self::React => &["src/index.*", "src/main.*", "src/App.*"],
            Self::Astro => &["src/pages/**", "src/middleware.*", "astro.config.*"],
            Self::NextJs => &[
                "pages/**",
                "src/pages/**",
                "app/**",
                "src/app/**",
                "middleware.*",
                "src/middleware.*",
            ],
            Self::FastApi => &["main.py", "app/main.py", "src/main.py", "**/routers/**"],
            Self::Django => &["manage.py", "**/views.py", "**/urls.py", "**/admin.py"],
        }
    }
}

impl fmt::Display for FrameworkId {
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
