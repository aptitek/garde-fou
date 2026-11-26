//! Automatic workspace context detector (`languages` × `features` × `frameworks`).

use crate::archetype_detector::{
    detect_extra_frameworks_into, detect_node_pkg_frameworks, detect_workspace_archetype,
};
use crate::discovery::peek_extensionless_shebang;
use gf_core::{
    ArchetypeSetting, Config, DomainKind, FeatureId, FrameworkId, Language, LanguageId,
    ProjectContext, RepoPath, YamlFlavor, classify_path, classify_yaml_flavor,
};
use gf_parsers::detect_language;
use std::{collections::BTreeSet, fs, path::Path};

#[rustfmt::skip]
const DOCKER_ROOT_MARKERS: &[&str] = &[
    "Dockerfile", "Containerfile", "compose.yaml", "compose.yml", "docker-compose.yml", "docker-compose.yaml",
];
const GIT_HOOK_MARKERS: &[&str] = &["lefthook.yml", ".lefthook.yml", ".pre-commit-config.yaml"];
#[rustfmt::skip]
const TASK_RUNNER_MARKERS: &[&str] = &["Taskfile.yml", "Taskfile.yaml", "justfile", "Makefile", "mise.toml"];
#[rustfmt::skip]
const ROOT_EXEMPT_MD: &[&str] = &["README.md", "AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "CONTRIBUTING.md", "LICENSE.md"];
#[rustfmt::skip]
const SOURCE_LANGUAGE_MAP: &[(Language, LanguageId)] = &[
    (Language::Rust, LanguageId::Rust), (Language::TypeScript, LanguageId::TypeScript),
    (Language::Tsx, LanguageId::TypeScript), (Language::JavaScript, LanguageId::TypeScript),
    (Language::Jsx, LanguageId::TypeScript), (Language::Python, LanguageId::Python),
    (Language::Go, LanguageId::Go), (Language::Kotlin, LanguageId::Kotlin), (Language::CSharp, LanguageId::CSharp),
    (Language::Shell, LanguageId::Shell), (Language::PowerShell, LanguageId::PowerShell),
    (Language::Yaml, LanguageId::Yaml), (Language::Css, LanguageId::Css),
    (Language::Xml, LanguageId::Xml), (Language::Svg, LanguageId::Xml),
    (Language::Toml, LanguageId::Toml), (Language::Sql, LanguageId::Sql),
    (Language::Astro, LanguageId::Astro), (Language::Html, LanguageId::Html),
];
const PANDA_CONFIGS: &[&str] = &["panda.config.ts", "panda.config.mjs", "panda.config.js"];
const WRANGLER_CONFIGS: &[&str] = &["wrangler.toml", "wrangler.jsonc", "wrangler.json"];
#[rustfmt::skip]
const DRIZZLE_CONFIGS: &[&str] = &["drizzle.config.ts", "drizzle.config.js", "drizzle.config.mjs"];
const RR_CONFIGS: &[&str] = &["react-router.config.ts", "react-router.config.js"];
const ASTRO_CONFIGS: &[&str] = &["astro.config.mjs", "astro.config.ts", "astro.config.js"];

#[rustfmt::skip]
fn any_file(root: &Path, names: &[&str]) -> bool { names.iter().any(|n| root.join(n).is_file()) }

#[rustfmt::skip]
fn has_m3e_markers(raw: &str) -> bool { raw.contains("primaryContainer") || raw.contains("onSurfaceVariant") || raw.contains("md.sys.") }

#[rustfmt::skip]
fn resolve_file_lang_id(root: &Path, path: &RepoPath) -> Option<LanguageId> {
    let lang = match detect_language(path) {
        Language::Unknown if path.extension().is_none() => peek_extensionless_shebang(&root.join(path.as_str())).unwrap_or(Language::Unknown),
        known => known,
    };
    if lang == Language::Markdown { return (!ROOT_EXEMPT_MD.contains(&path.as_str())).then_some(LanguageId::Markdown); }
    SOURCE_LANGUAGE_MAP.iter().find_map(|&(src, id)| (src == lang).then_some(id))
}

#[rustfmt::skip]
fn detect_manifest_languages(root: &Path, files: &[RepoPath], out: &mut BTreeSet<LanguageId>) {
    for (names, lid) in [
        (&["Cargo.toml"][..], LanguageId::Rust),
        (&["package.json", "tsconfig.json"][..], LanguageId::TypeScript),
        (&["pyproject.toml", "requirements.txt"][..], LanguageId::Python),
        (&["go.mod", "go.work"][..], LanguageId::Go),
        (&["build.gradle.kts", "build.gradle", "settings.gradle.kts"][..], LanguageId::Kotlin),
    ] {
        if any_file(root, names) { out.insert(lid); }
    }
    if files.iter().any(|p| matches!(p.extension(), Some("csproj" | "sln" | "slnx"))) { out.insert(LanguageId::CSharp); }
    if any_file(root, ASTRO_CONFIGS) { out.insert(LanguageId::Astro); out.insert(LanguageId::TypeScript); }
}

fn detect_languages_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<LanguageId>) {
    for path in files {
        if let Some(id) = resolve_file_lang_id(root, path) {
            out.insert(id);
            if id == LanguageId::Astro {
                out.insert(LanguageId::TypeScript);
            }
        }
    }
    detect_manifest_languages(root, files, out);
}

#[rustfmt::skip]
fn has_m3e_token_markers(root: &Path, files: &[RepoPath]) -> bool {
    PANDA_CONFIGS
        .iter()
        .any(|cfg| fs::read_to_string(root.join(cfg)).is_ok_and(|raw| has_m3e_markers(&raw)))
        || files.iter().any(|p| {
            (p.as_str().starts_with("tokens/") || p.as_str().starts_with("styles/"))
                && fs::read_to_string(root.join(p.as_str())).is_ok_and(|raw| has_m3e_markers(&raw))
        })
}

#[rustfmt::skip]
fn detect_web_features_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<FeatureId>) {
    let pkg = fs::read_to_string(root.join("package.json")).unwrap_or_default();
    if any_file(root, PANDA_CONFIGS) || pkg.contains("\"@pandacss/dev\"") {
        out.insert(FeatureId::PandaCss);
    }
    if has_m3e_token_markers(root, files) {
        out.insert(FeatureId::Material3Expressive);
    }
    if root.join(".htmlvalidate.json").is_file()
        || files.iter().any(|p| matches!(detect_language(p), Language::Html | Language::Astro))
    {
        out.insert(FeatureId::W3c);
    }
}

#[rustfmt::skip]
fn detect_features_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<FeatureId>) {
    if files.iter().any(|p| {
        detect_language(p) == Language::Dockerfile
            || matches!(classify_yaml_flavor(p), YamlFlavor::DockerCompose { .. })
    }) || any_file(root, DOCKER_ROOT_MARKERS) {
        out.insert(FeatureId::Docker);
    }
    if any_file(root, GIT_HOOK_MARKERS) || root.join(".husky").is_dir() {
        out.insert(FeatureId::GitHooks);
    }
    if root.join(".github/workflows").is_dir() {
        out.insert(FeatureId::CiGithub);
    }
    if any_file(root, TASK_RUNNER_MARKERS) {
        out.insert(FeatureId::TaskRunner);
    }
    detect_web_features_into(root, files, out);
}

#[rustfmt::skip]
fn detect_py_frameworks(raw: &str, out: &mut BTreeSet<FrameworkId>) {
    let lower = raw.to_ascii_lowercase();
    for (needle, fw) in [
        ("fastapi", FrameworkId::FastApi), ("django", FrameworkId::Django),
        ("nicegui", FrameworkId::NiceGui), ("torch", FrameworkId::PyTorch),
        ("safetensors", FrameworkId::PyTorch), ("tensorflow", FrameworkId::TensorFlow),
        ("keras", FrameworkId::TensorFlow), ("langchain", FrameworkId::LangChain),
        ("langgraph", FrameworkId::LangChain),
    ] {
        if lower.contains(needle) {
            out.insert(fw);
        }
    }
}

#[rustfmt::skip]
fn detect_manifest_frameworks(root: &Path, out: &mut BTreeSet<FrameworkId>) {
    if root.join("manage.py").is_file() { out.insert(FrameworkId::Django); }
    if let Ok(pkg_json) = fs::read_to_string(root.join("package.json")) {
        detect_node_pkg_frameworks(&pkg_json, out);
    }
    for py_manifest in ["pyproject.toml", "requirements.txt"] {
        if let Ok(raw) = fs::read_to_string(root.join(py_manifest)) { detect_py_frameworks(&raw, out); }
    }
}

fn detect_frameworks_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<FrameworkId>) {
    if root.join("Cargo.toml").is_file() {
        out.insert(FrameworkId::CargoWorkspace);
    }
    if any_file(root, ASTRO_CONFIGS) || files.iter().any(|p| detect_language(p) == Language::Astro)
    {
        out.insert(FrameworkId::Astro);
    }
    for (cfgs, fw) in [
        (RR_CONFIGS, FrameworkId::ReactRouter),
        (WRANGLER_CONFIGS, FrameworkId::CloudflareWorkers),
        (DRIZZLE_CONFIGS, FrameworkId::Drizzle),
    ] {
        if any_file(root, cfgs) {
            out.insert(fw);
        }
    }
    detect_manifest_frameworks(root, out);
}

#[rustfmt::skip]
fn read_dockerignore(root: &Path) -> Option<BTreeSet<String>> {
    let content = fs::read_to_string(root.join(".dockerignore")).ok()?;
    Some(
        content
            .lines()
            .filter_map(|line| {
                let t = line.trim();
                (!t.is_empty() && !t.starts_with('#') && !t.starts_with('!')).then_some(t)
            })
            .filter_map(|t| {
                let c = t.trim_start_matches("./").trim_start_matches('/').trim_end_matches("/**").trim_end_matches("/*").trim_end_matches('/');
                (!c.is_empty()).then(|| String::from(c))
            })
            .collect(),
    )
}

#[rustfmt::skip]
const DOMAIN_DIR_FEATURES: &[(&str, FeatureId)] = &[
    ("styles", FeatureId::Ui), ("tokens", FeatureId::Ui), ("locales", FeatureId::I18n),
    ("i18n", FeatureId::I18n), ("content", FeatureId::Content), ("courses", FeatureId::Content),
    (".storybook", FeatureId::Storybook),
];

#[rustfmt::skip]
fn detect_domain_features_into(root: &Path, files: &[RepoPath], config: &Config, out: &mut BTreeSet<FeatureId>) {
    for path in files {
        match classify_path(path, &config.domains) {
            DomainKind::Forme => { out.insert(FeatureId::Ui); }
            DomainKind::Texte => { out.insert(FeatureId::I18n); }
            DomainKind::Contenus => { out.insert(FeatureId::Content); }
            _ => {}
        }
    }
    for &(dir_name, feat) in DOMAIN_DIR_FEATURES {
        if root.join(dir_name).is_dir() {
            out.insert(feat);
        }
    }
}

/// Resolves the active [`ProjectContext`] by merging automatic workspace detection
/// with any explicit `[project]` configuration in `garde-fou.toml`.
#[must_use]
#[rustfmt::skip]
pub fn detect_project_context(root: &Path, discovered_files: &[RepoPath], config: &Config) -> ProjectContext {
    let mut ctx = ProjectContext {
        languages: config.project.languages.clone(),
        features: config.project.features.clone(),
        frameworks: config.project.frameworks.clone(),
        dockerignore_entries: read_dockerignore(root),
        ..ProjectContext::default()
    };
    detect_languages_into(root, discovered_files, &mut ctx.languages);
    detect_features_into(root, discovered_files, &mut ctx.features);
    detect_domain_features_into(root, discovered_files, config, &mut ctx.features);
    detect_frameworks_into(root, discovered_files, &mut ctx.frameworks);
    detect_extra_frameworks_into(root, discovered_files, &mut ctx.languages, &mut ctx.frameworks);
    if ctx.frameworks.contains(&FrameworkId::NiceGui) {
        ctx.features.insert(FeatureId::Ui);
    }
    if [FrameworkId::PyTorch, FrameworkId::TensorFlow, FrameworkId::LangChain].iter().any(|fw| ctx.frameworks.contains(fw)) {
        ctx.features.insert(FeatureId::AiMl);
    }
    ctx.archetype = match config.project.archetype {
        ArchetypeSetting::Explicit(arch) => arch,
        ArchetypeSetting::Auto => detect_workspace_archetype(root, discovered_files, &ctx),
    };
    ctx
}
