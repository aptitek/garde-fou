//! Automatic workspace context detector (`languages` × `features` × `frameworks`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use gf_core::{
    Config, DomainKind, FeatureId, FrameworkId, Language, LanguageId, ProjectContext, RepoPath,
    YamlFlavor, classify_path, classify_yaml_flavor,
};
use gf_parsers::detect_language;

use crate::discovery::peek_extensionless_shebang;

const DOCKER_ROOT_MARKERS: &[&str] = &[
    "Dockerfile",
    "Containerfile",
    "compose.yaml",
    "compose.yml",
    "docker-compose.yml",
    "docker-compose.yaml",
];

const GIT_HOOK_MARKERS: &[&str] = &["lefthook.yml", ".lefthook.yml", ".pre-commit-config.yaml"];

const TASK_RUNNER_MARKERS: &[&str] = &[
    "Taskfile.yml",
    "Taskfile.yaml",
    "justfile",
    "Makefile",
    "mise.toml",
];

const ROOT_EXEMPT_MD: &[&str] = &[
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE.md",
];

const SOURCE_LANGUAGE_MAP: &[(Language, LanguageId)] = &[
    (Language::Rust, LanguageId::Rust),
    (Language::TypeScript, LanguageId::TypeScript),
    (Language::Tsx, LanguageId::TypeScript),
    (Language::JavaScript, LanguageId::TypeScript),
    (Language::Jsx, LanguageId::TypeScript),
    (Language::Python, LanguageId::Python),
    (Language::Shell, LanguageId::Shell),
    (Language::PowerShell, LanguageId::PowerShell),
    (Language::Yaml, LanguageId::Yaml),
    (Language::Css, LanguageId::Css),
    (Language::Xml, LanguageId::Xml),
    (Language::Svg, LanguageId::Xml),
    (Language::Toml, LanguageId::Toml),
];

fn language_id_for_path(path: &RepoPath, lang: Language) -> Option<LanguageId> {
    if lang == Language::Markdown {
        return (!ROOT_EXEMPT_MD.contains(&path.as_str())).then_some(LanguageId::Markdown);
    }
    SOURCE_LANGUAGE_MAP
        .iter()
        .find_map(|&(src, id)| (src == lang).then_some(id))
}

fn detect_manifest_languages(root: &Path, out: &mut BTreeSet<LanguageId>) {
    if root.join("Cargo.toml").is_file() {
        out.insert(LanguageId::Rust);
    }
    if root.join("package.json").is_file() || root.join("tsconfig.json").is_file() {
        out.insert(LanguageId::TypeScript);
    }
    if root.join("pyproject.toml").is_file() || root.join("requirements.txt").is_file() {
        out.insert(LanguageId::Python);
    }
}

fn detect_languages_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<LanguageId>) {
    for path in files {
        let lang = match detect_language(path) {
            Language::Unknown if path.extension().is_none() => {
                peek_extensionless_shebang(&root.join(path.as_str())).unwrap_or(Language::Unknown)
            }
            known => known,
        };
        if let Some(id) = language_id_for_path(path, lang) {
            out.insert(id);
        }
    }
    detect_manifest_languages(root, out);
}

fn detect_features_into(root: &Path, files: &[RepoPath], out: &mut BTreeSet<FeatureId>) {
    let has_dockerfile_or_compose =
        files.iter().any(|p| {
            detect_language(p) == Language::Dockerfile
                || matches!(classify_yaml_flavor(p), YamlFlavor::DockerCompose { .. })
        }) || DOCKER_ROOT_MARKERS.iter().any(|m| root.join(m).is_file());
    if has_dockerfile_or_compose {
        out.insert(FeatureId::Docker);
    }
    if GIT_HOOK_MARKERS.iter().any(|m| root.join(m).is_file()) || root.join(".husky").is_dir() {
        out.insert(FeatureId::GitHooks);
    }
    if root.join(".github/workflows").is_dir() {
        out.insert(FeatureId::CiGithub);
    }
    if TASK_RUNNER_MARKERS.iter().any(|m| root.join(m).is_file()) {
        out.insert(FeatureId::TaskRunner);
    }
}

fn detect_node_frameworks(pkg_json: &str, out: &mut BTreeSet<FrameworkId>) {
    for (needle, fw) in [
        ("\"astro\"", FrameworkId::Astro),
        ("\"next\"", FrameworkId::NextJs),
        ("\"react\"", FrameworkId::React),
    ] {
        if pkg_json.contains(needle) {
            out.insert(fw);
        }
    }
}

fn detect_python_frameworks(manifest_text: &str, out: &mut BTreeSet<FrameworkId>) {
    let lower = manifest_text.to_ascii_lowercase();
    if lower.contains("fastapi") {
        out.insert(FrameworkId::FastApi);
    }
    if lower.contains("django") {
        out.insert(FrameworkId::Django);
    }
}

fn detect_frameworks_into(root: &Path, out: &mut BTreeSet<FrameworkId>) {
    if root.join("Cargo.toml").is_file() {
        out.insert(FrameworkId::CargoWorkspace);
    }
    if let Ok(pkg_json) = fs::read_to_string(root.join("package.json")) {
        detect_node_frameworks(&pkg_json, out);
    }
    for py_manifest in ["pyproject.toml", "requirements.txt"] {
        if let Ok(raw) = fs::read_to_string(root.join(py_manifest)) {
            detect_python_frameworks(&raw, out);
        }
    }
}

fn normalize_dockerignore_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return None;
    }
    let cleaned = trimmed
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches("/**")
        .trim_end_matches("/*")
        .trim_end_matches('/');
    (!cleaned.is_empty()).then(|| String::from(cleaned))
}

fn read_dockerignore(root: &Path) -> Option<BTreeSet<String>> {
    let content = fs::read_to_string(root.join(".dockerignore")).ok()?;
    let entries = content
        .lines()
        .filter_map(normalize_dockerignore_line)
        .collect();
    Some(entries)
}

const DOMAIN_DIR_FEATURES: &[(&str, FeatureId)] = &[
    ("styles", FeatureId::Ui),
    ("tokens", FeatureId::Ui),
    ("locales", FeatureId::I18n),
    ("i18n", FeatureId::I18n),
    ("content", FeatureId::Content),
    ("courses", FeatureId::Content),
    (".storybook", FeatureId::Storybook),
];

const fn feature_for_domain(domain: DomainKind) -> Option<FeatureId> {
    match domain {
        DomainKind::Forme => Some(FeatureId::Ui),
        DomainKind::Texte => Some(FeatureId::I18n),
        DomainKind::Contenus => Some(FeatureId::Content),
        _ => None,
    }
}

fn detect_domain_features_into(
    root: &Path,
    files: &[RepoPath],
    config: &Config,
    out: &mut BTreeSet<FeatureId>,
) {
    for path in files {
        if let Some(feat) = feature_for_domain(classify_path(path, &config.domains)) {
            out.insert(feat);
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
pub fn detect_project_context(
    root: &Path,
    discovered_files: &[RepoPath],
    config: &Config,
) -> ProjectContext {
    let mut languages = config.project.languages.clone();
    let mut features = config.project.features.clone();
    let mut frameworks = config.project.frameworks.clone();

    detect_languages_into(root, discovered_files, &mut languages);
    detect_features_into(root, discovered_files, &mut features);
    detect_domain_features_into(root, discovered_files, config, &mut features);
    detect_frameworks_into(root, &mut frameworks);

    ProjectContext {
        languages,
        features,
        frameworks,
        dockerignore_entries: read_dockerignore(root),
    }
}
