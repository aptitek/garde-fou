//! Pure choice catalogs (`Archetype`, archetype-filtered `Stack`, `Features`)
//! for the `garde init` wizard.

use gf_core::{FeatureId, FrameworkId, LanguageId, WorkspaceArchetype};

const LANG_SUMMARIES: [&str; 13] = [
    "Langage Rust (Edition 2024, sans unsafe/unwrap)",
    "TypeScript strict (AST & frontières UI)",
    "Python typé (pyproject.toml / PEP 723)",
    "Scripts POSIX sh / Bash (set -euo pipefail)",
    "PowerShell strict (Set-StrictMode -Version Latest)",
    "YAML 1.2 canonique, Compose & workflows CI",
    "Feuilles CSS & Design Tokens (Forme)",
    "Schémas XML & SVG assainis",
    "Manifestes TOML reproductibles & épinglés",
    "CommonMark / GFM éditorial pur",
    "Schémas SQL & migrations zéro-downtime",
    "Composants Astro SSR & islands",
    "Documents W3C HTML5 accessibles",
];

const FRAMEWORK_SUMMARIES: [&str; 13] = [
    "Workspace Cargo multi-crates DAG",
    "Composants React sémantiques",
    "Astro SSR & islands",
    "Next.js App/Pages Router",
    "Service ASGI FastAPI",
    "Application web Django",
    "React Router v7 Framework mode",
    "Cloudflare Workers Edge (D1/R2)",
    "Drizzle ORM typé",
    "Stack Docker Compose durcie",
    "Coffre Obsidian (.obsidian/ & .canvas)",
    "Publication scientifique Quarto (.qmd)",
    "Notebooks Jupyter reproductibles (.ipynb)",
];

/// Selectable stack item combining languages and frameworks in Step 2 of `WizardModel`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackToken {
    /// Programming or structured format language.
    Lang(LanguageId),
    /// Application or workspace framework.
    Framework(FrameworkId),
}

impl StackToken {
    /// Canonical slug displayed in the multi-select list.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Lang(id) => id.as_str(),
            Self::Framework(fw) => fw.as_str(),
        }
    }

    /// Short description displayed beside the stack choice.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Lang(id) => LANG_SUMMARIES[id as usize],
            Self::Framework(fw) => FRAMEWORK_SUMMARIES[fw as usize],
        }
    }
}

/// Ordered archetype choices presented in Step 1 (`Choose`).
pub const ARCHETYPE_CHOICES: [(WorkspaceArchetype, &str, &str); 6] = [
    (
        WorkspaceArchetype::Minimal,
        "Minimal",
        "Gouvernance sobre sans langage imposé (garde-fou.toml, AGENTS.md, garde.lock)",
    ),
    (
        WorkspaceArchetype::Software,
        "Software",
        "Ingénierie logicielle complète et ségrégation stricte des 6 domaines",
    ),
    (
        WorkspaceArchetype::Infra,
        "Infra / Docker Compose",
        "Stacks conteneurisées (compose.yaml, Dockerfile, CI durcie)",
    ),
    (
        WorkspaceArchetype::Knowledge,
        "Knowledge / Obsidian",
        "Base de connaissances Markdown & JSON Canvas (.obsidian/)",
    ),
    (
        WorkspaceArchetype::Academic,
        "Academic / Jupyter & Quarto",
        "Cours et notebooks scientifiques reproductibles (.ipynb, .qmd)",
    ),
    (
        WorkspaceArchetype::Scripts,
        "Scripts pratico-pratiques",
        "Automatisation Shell, PowerShell et scripts Python PEP 723",
    ),
];

/// Ordered 12 feature choices presented in Step 3 (`MultiSelect`).
pub const FEATURE_CHOICES: [(FeatureId, &str); 12] = [
    (
        FeatureId::Docker,
        "Conteneurisation durcie (Dockerfile & .dockerignore)",
    ),
    (
        FeatureId::GitHooks,
        "Portes locales lefthook (pre-commit, commit-msg, pre-push)",
    ),
    (
        FeatureId::TaskRunner,
        "Workflows reproductibles (Taskfile.yml & mise.toml)",
    ),
    (
        FeatureId::CiGithub,
        "GitHub Actions épinglées par SHA/tag (GF080)",
    ),
    (
        FeatureId::Ui,
        "Design Tokens & ségrégation Forme (styles/tokens.css)",
    ),
    (FeatureId::PandaCss, "Extraction Panda CSS zéro-runtime"),
    (
        FeatureId::Material3Expressive,
        "Tokens Material Design 3 Expressive (m3e)",
    ),
    (FeatureId::W3c, "Conformité W3C HTML5 & WCAG 2.2 AA"),
    (
        FeatureId::I18n,
        "Dictionnaires Texte symétriques (locales/en.json, fr.json)",
    ),
    (
        FeatureId::Content,
        "Domaine Contenus Markdown pur (content/index.md)",
    ),
    (
        FeatureId::Tdd,
        "Parité stricte 1:1 tests compagnons (GF076)",
    ),
    (
        FeatureId::Storybook,
        "Parité composants .stories.tsx (GF076)",
    ),
];

/// Returns the smart pre-filtered [`StackToken`] list tailored to `archetype`.
#[must_use]
pub fn stack_choices_for(archetype: WorkspaceArchetype) -> Vec<StackToken> {
    match archetype {
        WorkspaceArchetype::Minimal => vec![
            StackToken::Lang(LanguageId::Toml),
            StackToken::Lang(LanguageId::Markdown),
            StackToken::Lang(LanguageId::Yaml),
        ],
        WorkspaceArchetype::Software => vec![
            StackToken::Lang(LanguageId::Rust),
            StackToken::Framework(FrameworkId::CargoWorkspace),
            StackToken::Lang(LanguageId::TypeScript),
            StackToken::Framework(FrameworkId::React),
            StackToken::Framework(FrameworkId::Astro),
            StackToken::Framework(FrameworkId::NextJs),
            StackToken::Framework(FrameworkId::ReactRouter),
            StackToken::Framework(FrameworkId::CloudflareWorkers),
            StackToken::Framework(FrameworkId::Drizzle),
            StackToken::Lang(LanguageId::Python),
            StackToken::Framework(FrameworkId::FastApi),
            StackToken::Framework(FrameworkId::Django),
            StackToken::Lang(LanguageId::Sql),
        ],
        WorkspaceArchetype::Infra => vec![
            StackToken::Lang(LanguageId::Yaml),
            StackToken::Framework(FrameworkId::DockerCompose),
            StackToken::Lang(LanguageId::Shell),
            StackToken::Lang(LanguageId::PowerShell),
            StackToken::Lang(LanguageId::Toml),
        ],
        WorkspaceArchetype::Knowledge => vec![
            StackToken::Lang(LanguageId::Markdown),
            StackToken::Framework(FrameworkId::Obsidian),
            StackToken::Lang(LanguageId::Yaml),
        ],
        WorkspaceArchetype::Academic => vec![
            StackToken::Lang(LanguageId::Python),
            StackToken::Framework(FrameworkId::Jupyter),
            StackToken::Framework(FrameworkId::Quarto),
            StackToken::Lang(LanguageId::Markdown),
            StackToken::Lang(LanguageId::Yaml),
        ],
        WorkspaceArchetype::Scripts => vec![
            StackToken::Lang(LanguageId::Shell),
            StackToken::Lang(LanguageId::PowerShell),
            StackToken::Lang(LanguageId::Python),
            StackToken::Lang(LanguageId::Yaml),
        ],
    }
}
