//! Ordered catalogs and [`ConfigCategory`] navigation metadata for Tab `[2] Projet & Garde-fous`.

use gf_core::{
    ArchetypeSetting, FeatureId, FrameworkId, GovernanceFlag, LanguageId, WorkspaceArchetype,
};

/// Ordered list of all 16 languages displayed in the Languages checkbox section.
pub const ALL_LANGUAGES: [LanguageId; 16] = [
    LanguageId::Rust,
    LanguageId::TypeScript,
    LanguageId::Python,
    LanguageId::Go,
    LanguageId::Kotlin,
    LanguageId::CSharp,
    LanguageId::Shell,
    LanguageId::PowerShell,
    LanguageId::Yaml,
    LanguageId::Sql,
    LanguageId::Toml,
    LanguageId::Markdown,
    LanguageId::Css,
    LanguageId::Xml,
    LanguageId::Astro,
    LanguageId::Html,
];

/// Ordered list of all 16 frameworks displayed in the Frameworks checkbox section.
pub const ALL_FRAMEWORKS: [FrameworkId; 16] = [
    FrameworkId::CargoWorkspace,
    FrameworkId::DockerCompose,
    FrameworkId::NodeJs,
    FrameworkId::Express,
    FrameworkId::Mongoose,
    FrameworkId::React,
    FrameworkId::ReactRouter,
    FrameworkId::Astro,
    FrameworkId::NextJs,
    FrameworkId::CloudflareWorkers,
    FrameworkId::Drizzle,
    FrameworkId::FastApi,
    FrameworkId::Django,
    FrameworkId::Obsidian,
    FrameworkId::Quarto,
    FrameworkId::Jupyter,
];

/// Ordered list of all 12 features displayed in the Features checkbox section.
pub const ALL_FEATURES: [FeatureId; 12] = [
    FeatureId::Docker,
    FeatureId::GitHooks,
    FeatureId::TaskRunner,
    FeatureId::CiGithub,
    FeatureId::Ui,
    FeatureId::PandaCss,
    FeatureId::Material3Expressive,
    FeatureId::W3c,
    FeatureId::I18n,
    FeatureId::Content,
    FeatureId::Tdd,
    FeatureId::Storybook,
];

/// Ordered list of all 7 archetype radio choices (`Auto` + 6 explicit archetypes).
pub const ALL_ARCHETYPE_SETTINGS: [ArchetypeSetting; 7] = [
    ArchetypeSetting::Auto,
    ArchetypeSetting::Explicit(WorkspaceArchetype::Minimal),
    ArchetypeSetting::Explicit(WorkspaceArchetype::Scripts),
    ArchetypeSetting::Explicit(WorkspaceArchetype::Infra),
    ArchetypeSetting::Explicit(WorkspaceArchetype::Knowledge),
    ArchetypeSetting::Explicit(WorkspaceArchetype::Academic),
    ArchetypeSetting::Explicit(WorkspaceArchetype::Software),
];

/// Selectable category sections inside Tab `[2] Projet & Garde-fous`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConfigCategory {
    /// Exclusive radio buttons for `ArchetypeSetting`.
    #[default]
    Archetype = 0,
    /// Multi-select checkboxes for active languages.
    Languages = 1,
    /// Multi-select checkboxes for active frameworks.
    Frameworks = 2,
    /// Multi-select checkboxes for active features & rules.
    Features = 3,
    /// Multi-select checkboxes for governance flags.
    Governance = 4,
}

impl ConfigCategory {
    /// All 5 categories in left-to-right display order.
    pub const ALL: [Self; 5] = [
        Self::Archetype,
        Self::Languages,
        Self::Frameworks,
        Self::Features,
        Self::Governance,
    ];

    /// Short tab label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        const LABELS: [&str; 5] = [
            "Archétype",
            "Langages",
            "Frameworks",
            "Features",
            "Gouvernance",
        ];
        LABELS[self as usize]
    }

    /// Number of selectable rows inside this category.
    #[must_use]
    pub const fn len(self) -> usize {
        match self {
            Self::Archetype => ALL_ARCHETYPE_SETTINGS.len(),
            Self::Languages => ALL_LANGUAGES.len(),
            Self::Frameworks => ALL_FRAMEWORKS.len(),
            Self::Features => ALL_FEATURES.len(),
            Self::Governance => GovernanceFlag::ALL.len(),
        }
    }

    /// Returns `true` when the category has zero rows (always `false` for built-in categories).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Cycles to the next category.
    #[must_use]
    pub const fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    /// Cycles to the previous category.
    #[must_use]
    pub const fn prev(self) -> Self {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}
