//! Pure extension, `Dockerfile` basename, shebang, and manifest language detectors.

use gf_core::{Language, RepoPath, is_binary_asset_extension};

use crate::langs::shell::dialect::detect_shebang_language;
use crate::types::ProjectMarker;

const EXT_LANGUAGES: &[(&str, Language)] = &[
    ("rs", Language::Rust),
    ("py", Language::Python),
    ("ts", Language::TypeScript),
    ("mts", Language::TypeScript),
    ("cts", Language::TypeScript),
    ("tsx", Language::Tsx),
    ("js", Language::JavaScript),
    ("mjs", Language::JavaScript),
    ("cjs", Language::JavaScript),
    ("jsx", Language::Jsx),
    ("go", Language::Go),
    ("kt", Language::Kotlin),
    ("kts", Language::Kotlin),
    ("cs", Language::CSharp),
    ("dockerfile", Language::Dockerfile),
    ("containerfile", Language::Dockerfile),
    ("css", Language::Css),
    ("scss", Language::Css),
    ("md", Language::Markdown),
    ("mdx", Language::Markdown),
    ("json", Language::Json),
    ("jsonc", Language::Json),
    ("arb", Language::Json),
    ("po", Language::GettextPo),
    ("pot", Language::GettextPo),
    ("ftl", Language::FluentFtl),
    ("mmd", Language::Mermaid),
    ("mermaid", Language::Mermaid),
    ("puml", Language::PlantUml),
    ("plantuml", Language::PlantUml),
    ("iuml", Language::PlantUml),
    ("pu", Language::PlantUml),
    ("toml", Language::Toml),
    ("yml", Language::Yaml),
    ("yaml", Language::Yaml),
    ("sh", Language::Shell),
    ("bash", Language::Shell),
    ("ps1", Language::PowerShell),
    ("psm1", Language::PowerShell),
    ("psd1", Language::PowerShell),
    ("svg", Language::Svg),
    ("xml", Language::Xml),
    ("csproj", Language::Xml),
    ("props", Language::Xml),
    ("sql", Language::Sql),
    ("pls", Language::PlSql),
    ("plb", Language::PlSql),
    ("pks", Language::PlSql),
    ("pkb", Language::PlSql),
    ("prc", Language::PlSql),
    ("fnc", Language::PlSql),
    ("trg", Language::PlSql),
    ("tps", Language::PlSql),
    ("tpb", Language::PlSql),
    ("astro", Language::Astro),
    ("html", Language::Html),
    ("htm", Language::Html),
    ("ipynb", Language::Notebook),
    ("qmd", Language::Quarto),
    ("canvas", Language::JsonCanvas),
];

fn is_dockerfile_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower == "dockerfile"
        || lower == "containerfile"
        || lower.starts_with("dockerfile.")
        || lower.starts_with("containerfile.")
}

/// Infers the [`Language`] of a repository file from its name or extension.
#[must_use]
pub fn detect_language(path: &RepoPath) -> Language {
    let file_name = path.file_name();
    if file_name.eq_ignore_ascii_case("dockerfile")
        || file_name.eq_ignore_ascii_case("containerfile")
    {
        return Language::Dockerfile;
    }
    if let Some(ext) = path.extension() {
        if let Some((_, lang)) = EXT_LANGUAGES
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(ext))
        {
            return *lang;
        }
        if is_binary_asset_extension(ext) {
            return Language::BinaryAsset;
        }
    }
    if is_dockerfile_name(file_name) {
        return Language::Dockerfile;
    }
    Language::Unknown
}

/// Resolves [`Language`] from `path`, falling back to line-1 `#!` shebang inspection
/// for extensionless scripts (`#!/bin/sh`, `#!/usr/bin/env bash`, `#!/usr/bin/env pwsh`).
#[must_use]
pub fn detect_language_or_shebang(path: &RepoPath, source: &str) -> Language {
    let by_path = detect_language(path);
    if by_path != Language::Unknown {
        return by_path;
    }
    if path.extension().is_none()
        && let Some(first_line) = source.lines().next()
        && let Some(lang) = detect_shebang_language(first_line)
    {
        return lang;
    }
    Language::Unknown
}

/// Detects well-known project ecosystem manifest files from a [`RepoPath`].
#[must_use]
pub fn detect_project_marker(path: &RepoPath) -> Option<ProjectMarker> {
    if detect_language(path) == Language::Dockerfile {
        return Some(ProjectMarker::Containerfile);
    }
    match path.file_name() {
        "Cargo.toml" => Some(ProjectMarker::CargoManifest),
        "package.json" => Some(ProjectMarker::NodeManifest),
        "go.mod" => Some(ProjectMarker::GoMod),
        _ => None,
    }
}
