//! Compile-time embedded Markdown fragments (`content/agent-fragments/*.md`)
//! keeping the `garde` binary 100% standalone while auditing fragments via `GF063`.

use crate::archetype_policy::WorkspaceArchetype;
use crate::context::{FeatureId, FrameworkId, LanguageId};

/// Embedded `.agents/skills/garde-fou/SKILL.md` always available offline.
pub const EMBEDDED_GARDE_FOU_SKILL: &str =
    include_str!("../../../content/agent-fragments/skill-garde-fou.md");

/// Core zero-debt SRP/DRY/KISS/YAGNI fragment.
pub const CORE_FRAGMENT: &str = include_str!("../../../content/agent-fragments/core.md");

/// Six-domain segregation fragment (`GF060`–`GF066`).
pub const DOMAINS_FRAGMENT: &str = include_str!("../../../content/agent-fragments/domains.md");

const LANGUAGE_FRAGMENTS: [&str; 17] = [
    include_str!("../../../content/agent-fragments/lang-rust.md"),
    include_str!("../../../content/agent-fragments/lang-typescript.md"),
    include_str!("../../../content/agent-fragments/lang-python.md"),
    include_str!("../../../content/agent-fragments/lang-shell.md"),
    include_str!("../../../content/agent-fragments/lang-powershell.md"),
    include_str!("../../../content/agent-fragments/lang-yaml.md"),
    include_str!("../../../content/agent-fragments/lang-css.md"),
    include_str!("../../../content/agent-fragments/lang-xml.md"),
    include_str!("../../../content/agent-fragments/lang-toml.md"),
    include_str!("../../../content/agent-fragments/lang-markdown.md"),
    include_str!("../../../content/agent-fragments/lang-sql.md"),
    include_str!("../../../content/agent-fragments/lang-astro.md"),
    include_str!("../../../content/agent-fragments/lang-html.md"),
    include_str!("../../../content/agent-fragments/lang-go.md"),
    include_str!("../../../content/agent-fragments/lang-kotlin.md"),
    include_str!("../../../content/agent-fragments/lang-csharp.md"),
    include_str!("../../../content/agent-fragments/lang-plsql.md"),
];

const FEATURE_FRAGMENTS: [&str; 13] = [
    include_str!("../../../content/agent-fragments/feat-docker.md"),
    include_str!("../../../content/agent-fragments/feat-git-hooks.md"),
    include_str!("../../../content/agent-fragments/feat-ci-github.md"),
    include_str!("../../../content/agent-fragments/feat-task-runner.md"),
    include_str!("../../../content/agent-fragments/feat-ui.md"),
    include_str!("../../../content/agent-fragments/feat-i18n.md"),
    include_str!("../../../content/agent-fragments/feat-content.md"),
    include_str!("../../../content/agent-fragments/feat-tdd.md"),
    include_str!("../../../content/agent-fragments/feat-storybook.md"),
    include_str!("../../../content/agent-fragments/feat-panda-css.md"),
    include_str!("../../../content/agent-fragments/feat-m3e.md"),
    include_str!("../../../content/agent-fragments/feat-w3c-a11y.md"),
    include_str!("../../../content/agent-fragments/feat-ai-ml.md"),
];

const FRAMEWORK_FRAGMENTS: [&str; 22] = [
    include_str!("../../../content/agent-fragments/fw-cargo-workspace.md"),
    include_str!("../../../content/agent-fragments/fw-react.md"),
    include_str!("../../../content/agent-fragments/fw-astro.md"),
    include_str!("../../../content/agent-fragments/fw-nextjs.md"),
    include_str!("../../../content/agent-fragments/fw-fastapi.md"),
    include_str!("../../../content/agent-fragments/fw-django.md"),
    include_str!("../../../content/agent-fragments/fw-react-router.md"),
    include_str!("../../../content/agent-fragments/fw-cloudflare-workers.md"),
    include_str!("../../../content/agent-fragments/fw-drizzle.md"),
    include_str!("../../../content/agent-fragments/fw-docker-compose.md"),
    include_str!("../../../content/agent-fragments/fw-obsidian.md"),
    include_str!("../../../content/agent-fragments/fw-quarto.md"),
    include_str!("../../../content/agent-fragments/fw-jupyter.md"),
    include_str!("../../../content/agent-fragments/fw-nodejs.md"),
    include_str!("../../../content/agent-fragments/fw-mern.md"),
    include_str!("../../../content/agent-fragments/fw-mern.md"),
    include_str!("../../../content/agent-fragments/fw-nicegui.md"),
    include_str!("../../../content/agent-fragments/feat-ai-ml.md"),
    include_str!("../../../content/agent-fragments/feat-ai-ml.md"),
    include_str!("../../../content/agent-fragments/feat-ai-ml.md"),
    include_str!("../../../content/agent-fragments/fw-postgresql.md"),
    include_str!("../../../content/agent-fragments/fw-mariadb.md"),
];

const ARCH_MINIMAL_FRAGMENT: &str =
    include_str!("../../../content/agent-fragments/arch-minimal.md");
const ARCH_SCRIPTS_FRAGMENT: &str =
    include_str!("../../../content/agent-fragments/arch-scripts.md");
const ARCH_INFRA_FRAGMENT: &str = include_str!("../../../content/agent-fragments/arch-infra.md");
const ARCH_ACADEMIC_FRAGMENT: &str =
    include_str!("../../../content/agent-fragments/arch-academic.md");

/// Returns the optional archetype-specific Markdown fragment for `archetype`.
#[must_use]
pub const fn archetype_fragment(archetype: WorkspaceArchetype) -> Option<&'static str> {
    match archetype {
        WorkspaceArchetype::Minimal => Some(ARCH_MINIMAL_FRAGMENT),
        WorkspaceArchetype::Scripts => Some(ARCH_SCRIPTS_FRAGMENT),
        WorkspaceArchetype::Infra => Some(ARCH_INFRA_FRAGMENT),
        WorkspaceArchetype::Academic => Some(ARCH_ACADEMIC_FRAGMENT),
        WorkspaceArchetype::Knowledge | WorkspaceArchetype::Software => None,
    }
}

/// Returns the embedded Markdown fragment for `lang`.
#[must_use]
pub const fn language_fragment(lang: LanguageId) -> &'static str {
    LANGUAGE_FRAGMENTS[lang as usize]
}

/// Returns the embedded Markdown fragment for `feat`.
#[must_use]
pub const fn feature_fragment(feat: FeatureId) -> &'static str {
    FEATURE_FRAGMENTS[feat as usize]
}

/// Returns the embedded Markdown fragment for `fw`.
#[must_use]
pub const fn framework_fragment(fw: FrameworkId) -> &'static str {
    FRAMEWORK_FRAGMENTS[fw as usize]
}
