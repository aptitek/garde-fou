//! Deterministic auto-detection of [`WorkspaceArchetype`] and `Academic`/`Knowledge`/`Infra`
//! frameworks (`Quarto`, `Jupyter`, `Obsidian`, `DockerCompose`).

use std::collections::BTreeSet;
use std::path::Path;

use gf_core::{
    FeatureId, FrameworkId, Language, LanguageId, ProjectContext, RepoPath, WorkspaceArchetype,
    YamlFlavor, classify_yaml_flavor, is_root_governance_doc,
};
use gf_parsers::detect_language;

const QUARTO_CONFIGS: &[&str] = &["_quarto.yml", "_quarto.yaml"];
const COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yml",
    "docker-compose.yaml",
];
const DOCKER_FILES: &[&str] = &["Dockerfile", "Containerfile"];

fn any_root_file(root: &Path, names: &[&str]) -> bool {
    names.iter().any(|n| root.join(n).is_file())
}

const PRIMARY_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "go.work",
    "build.gradle.kts",
    "build.gradle",
    "settings.gradle.kts",
];

fn is_primary_source_file(p: &RepoPath) -> bool {
    let is_dotnet = p.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("csproj")
            || e.eq_ignore_ascii_case("sln")
            || e.eq_ignore_ascii_case("slnx")
    });
    is_dotnet
        || matches!(
            detect_language(p),
            Language::Rust
                | Language::TypeScript
                | Language::Tsx
                | Language::JavaScript
                | Language::Jsx
                | Language::Astro
                | Language::Go
                | Language::Kotlin
                | Language::CSharp
        )
}

fn has_python_app_layout(root: &Path, files: &[RepoPath]) -> bool {
    root.join("pyproject.toml").is_file()
        && (root.join("src").is_dir()
            || root.join("app").is_dir()
            || files
                .iter()
                .any(|p| p.as_str().starts_with("src/") || p.as_str().starts_with("app/")))
}

fn has_primary_app_source(root: &Path, files: &[RepoPath]) -> bool {
    any_root_file(root, PRIMARY_MANIFESTS)
        || root.join("crates").is_dir()
        || files.iter().any(is_primary_source_file)
        || has_python_app_layout(root, files)
}

/// Detects `Quarto`, `Jupyter`, `Obsidian`, and `DockerCompose` frameworks from workspace files.
pub fn detect_extra_frameworks_into(
    root: &Path,
    files: &[RepoPath],
    languages: &mut BTreeSet<LanguageId>,
    frameworks: &mut BTreeSet<FrameworkId>,
) {
    if any_root_file(root, QUARTO_CONFIGS)
        || files.iter().any(|p| detect_language(p) == Language::Quarto)
    {
        frameworks.insert(FrameworkId::Quarto);
    }
    if files
        .iter()
        .any(|p| detect_language(p) == Language::Notebook)
    {
        frameworks.insert(FrameworkId::Jupyter);
        languages.insert(LanguageId::Python);
    }
    if root.join(".obsidian").is_dir()
        || files
            .iter()
            .any(|p| detect_language(p) == Language::JsonCanvas)
    {
        frameworks.insert(FrameworkId::Obsidian);
    }
    if any_root_file(root, COMPOSE_FILES)
        || files
            .iter()
            .any(|p| matches!(classify_yaml_flavor(p), YamlFlavor::DockerCompose { .. }))
    {
        frameworks.insert(FrameworkId::DockerCompose);
    }
}

fn has_knowledge_only_files(files: &[RepoPath]) -> bool {
    let mut saw_note = false;
    for p in files {
        let lang = detect_language(p);
        if matches!(lang, Language::Markdown | Language::JsonCanvas) && !is_root_governance_doc(p) {
            saw_note = true;
        } else if lang.is_code_analyzed() {
            return false;
        }
    }
    saw_note
}

fn has_standalone_scripts(
    files: &[RepoPath],
    languages: &BTreeSet<LanguageId>,
    frameworks: &BTreeSet<FrameworkId>,
) -> bool {
    let has_script_file = files.iter().any(|p| {
        matches!(
            detect_language(p),
            Language::Shell | Language::PowerShell | Language::Python
        )
    });
    let explicit_shell_lang = (languages.contains(&LanguageId::Shell)
        || languages.contains(&LanguageId::PowerShell))
        && frameworks.is_empty();
    has_script_file || explicit_shell_lang
}

fn is_knowledge_workspace(root: &Path, files: &[RepoPath], ctx: &ProjectContext) -> bool {
    root.join(".obsidian").is_dir()
        || ctx.frameworks.contains(&FrameworkId::Obsidian)
        || has_knowledge_only_files(files)
}

fn is_infra_workspace(root: &Path, ctx: &ProjectContext) -> bool {
    ctx.frameworks.contains(&FrameworkId::DockerCompose)
        || any_root_file(root, COMPOSE_FILES)
        || any_root_file(root, DOCKER_FILES)
        || ctx.features.contains(&FeatureId::Docker)
}

/// Deterministically classifies a repository into one of the 6 proportional [`WorkspaceArchetype`]s.
#[must_use]
pub fn detect_workspace_archetype(
    root: &Path,
    files: &[RepoPath],
    ctx: &ProjectContext,
) -> WorkspaceArchetype {
    let has_app = has_primary_app_source(root, files);
    let has_academic_fw = ctx.frameworks.contains(&FrameworkId::Quarto)
        || ctx.frameworks.contains(&FrameworkId::Jupyter);
    if has_academic_fw && !has_app {
        return WorkspaceArchetype::Academic;
    }
    if has_app {
        return WorkspaceArchetype::Software;
    }
    if is_knowledge_workspace(root, files, ctx) {
        return WorkspaceArchetype::Knowledge;
    }
    if is_infra_workspace(root, ctx) {
        return WorkspaceArchetype::Infra;
    }
    if has_standalone_scripts(files, &ctx.languages, &ctx.frameworks) {
        return WorkspaceArchetype::Scripts;
    }
    WorkspaceArchetype::Minimal
}
