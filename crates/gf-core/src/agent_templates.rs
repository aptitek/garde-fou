//! Compile-time embedded Markdown fragments (`content/agent-fragments/*.md`)
//! keeping the `garde` binary 100% standalone while auditing fragments via `GF063`.

use crate::context::{FeatureId, FrameworkId, LanguageId};

/// Embedded `.agents/skills/garde-fou/SKILL.md` always available offline.
pub const EMBEDDED_GARDE_FOU_SKILL: &str =
    include_str!("../../../content/agent-fragments/skill-garde-fou.md");

/// Core zero-debt SRP/DRY/KISS/YAGNI fragment.
pub const CORE_FRAGMENT: &str = include_str!("../../../content/agent-fragments/core.md");

/// Six-domain segregation fragment (`GF060`–`GF066`).
pub const DOMAINS_FRAGMENT: &str = include_str!("../../../content/agent-fragments/domains.md");

/// Returns the embedded Markdown fragment for `lang`.
#[must_use]
pub const fn language_fragment(lang: LanguageId) -> &'static str {
    match lang {
        LanguageId::Rust => include_str!("../../../content/agent-fragments/lang-rust.md"),
        LanguageId::TypeScript => {
            include_str!("../../../content/agent-fragments/lang-typescript.md")
        }
        LanguageId::Python => include_str!("../../../content/agent-fragments/lang-python.md"),
    }
}

/// Returns the embedded Markdown fragment for `feat`.
#[must_use]
pub const fn feature_fragment(feat: FeatureId) -> &'static str {
    match feat {
        FeatureId::Docker => include_str!("../../../content/agent-fragments/feat-docker.md"),
        FeatureId::GitHooks => include_str!("../../../content/agent-fragments/feat-git-hooks.md"),
        FeatureId::CiGithub => include_str!("../../../content/agent-fragments/feat-ci-github.md"),
        FeatureId::TaskRunner => {
            include_str!("../../../content/agent-fragments/feat-task-runner.md")
        }
        FeatureId::Ui => include_str!("../../../content/agent-fragments/feat-ui.md"),
        FeatureId::I18n => include_str!("../../../content/agent-fragments/feat-i18n.md"),
        FeatureId::Content => include_str!("../../../content/agent-fragments/feat-content.md"),
        FeatureId::Tdd => include_str!("../../../content/agent-fragments/feat-tdd.md"),
        FeatureId::Storybook => include_str!("../../../content/agent-fragments/feat-storybook.md"),
    }
}

/// Returns the embedded Markdown fragment for `fw`.
#[must_use]
pub const fn framework_fragment(fw: FrameworkId) -> &'static str {
    match fw {
        FrameworkId::CargoWorkspace => {
            include_str!("../../../content/agent-fragments/fw-cargo-workspace.md")
        }
        FrameworkId::React => include_str!("../../../content/agent-fragments/fw-react.md"),
        FrameworkId::Astro => include_str!("../../../content/agent-fragments/fw-astro.md"),
        FrameworkId::NextJs => include_str!("../../../content/agent-fragments/fw-nextjs.md"),
        FrameworkId::FastApi => include_str!("../../../content/agent-fragments/fw-fastapi.md"),
        FrameworkId::Django => include_str!("../../../content/agent-fragments/fw-django.md"),
    }
}
