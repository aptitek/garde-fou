//! Pure `#![no_std]` composable `.gitignore` and `.dockerignore` generators with
//! non-destructive managed-block merging.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::context::{FeatureId, FrameworkId, LanguageId, ProjectContext};

/// Opening marker of the Garde-fou managed `.gitignore` block.
pub const MANAGED_START: &str = "# >>> garde-fou managed >>>";
/// Closing marker of the Garde-fou managed `.gitignore` block.
pub const MANAGED_END: &str = "# <<< garde-fou managed <<<";

const BASE_OS_EDITOR: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    "*.swp",
    "*.swo",
    "*~",
    ".idea/",
    ".vscode/*",
    "!.vscode/extensions.json",
];

const BASE_GARDE_FOU_COV: &[&str] = &[
    ".garde-fou-cache/",
    "lcov.info",
    "coverage/",
    "*.profraw",
    "*.profdata",
    "mutants.out*/",
    "reports/mutation/",
    ".stryker-tmp/",
    "StrykerOutput/",
    ".mutmut-cache",
    "build/reports/pitest/",
    "gremlins-report.json",
    "infection-report.json",
    "infection*.log",
];

const BASE_SECRETS_ENV: &[&str] = &[".env", ".env.*", "!.env.example", "*.pem", "*.key"];

fn append_section(out: &mut String, title: &str, entries: &[&str], seen: &mut BTreeSet<String>) {
    let mut fresh = Vec::new();
    for &entry in entries {
        if seen.insert(String::from(entry)) {
            fresh.push(entry);
        }
    }
    if fresh.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("# ");
    out.push_str(title);
    out.push('\n');
    for item in fresh {
        out.push_str(item);
        out.push('\n');
    }
}

#[rustfmt::skip]
struct IgnoreSpec<K: Copy> { key: K, title: &'static str, entries: &'static [&'static str] }

#[rustfmt::skip]
impl<K: Copy> IgnoreSpec<K> {
    const fn new(key: K, title: &'static str, entries: &'static [&'static str]) -> Self {
        Self { key, title, entries }
    }
}

#[rustfmt::skip]
const LANG_IGNORES: &[IgnoreSpec<LanguageId>] = &[
    IgnoreSpec::new(LanguageId::Rust, "Rust", &["/target/", "**/*.rs.bk"]),
    IgnoreSpec::new(LanguageId::Python, "Python", &[".venv/", "__pycache__/", "*.py[cod]", ".pytest_cache/", ".mypy_cache/", ".ruff_cache/"]),
    IgnoreSpec::new(LanguageId::Go, "Go", &["bin/", "*.test", "*.out"]),
    IgnoreSpec::new(LanguageId::Kotlin, "Kotlin / Gradle", &[".gradle/", "build/", ".kotlin/"]),
    IgnoreSpec::new(LanguageId::CSharp, "C# / .NET", &["bin/", "obj/", "*.user"]),
];

fn append_language_sections(out: &mut String, ctx: &ProjectContext, seen: &mut BTreeSet<String>) {
    if ctx.languages.contains(&LanguageId::TypeScript) || ctx.languages.contains(&LanguageId::Astro)
    {
        append_section(
            out,
            "TypeScript / Node",
            &[
                "node_modules/",
                ".pnpm-store/",
                "dist/",
                "*.tsbuildinfo",
                ".tsbuildinfo",
                "styled-system/",
                ".wrangler/",
                ".dev.vars",
            ],
            seen,
        );
    }
    for spec in LANG_IGNORES {
        if ctx.languages.contains(&spec.key) {
            append_section(out, spec.title, spec.entries, seen);
        }
    }
}

#[rustfmt::skip]
const FRAMEWORK_IGNORES: &[IgnoreSpec<FrameworkId>] = &[
    IgnoreSpec::new(FrameworkId::Astro, "Astro", &[".astro/"]),
    IgnoreSpec::new(FrameworkId::NextJs, "Next.js", &[".next/", "out/"]),
    IgnoreSpec::new(FrameworkId::ReactRouter, "React Router", &[".react-router/", "build/"]),
    IgnoreSpec::new(FrameworkId::CloudflareWorkers, "Cloudflare Workers", &[".wrangler/", ".dev.vars"]),
    IgnoreSpec::new(FrameworkId::Django, "Django", &["db.sqlite3", "staticfiles/"]),
    IgnoreSpec::new(FrameworkId::Jupyter, "Jupyter", &[".ipynb_checkpoints/", "/.virtual_documents/"]),
    IgnoreSpec::new(FrameworkId::Quarto, "Quarto", &["/.quarto/", "/_site/", "/_book/", "/*_files/"]),
    IgnoreSpec::new(FrameworkId::Obsidian, "Obsidian", &[".obsidian/workspace.json", ".obsidian/workspace-mobile.json", ".obsidian/cache", ".trash/"]),
];

fn append_framework_sections(out: &mut String, ctx: &ProjectContext, seen: &mut BTreeSet<String>) {
    if ctx.languages.contains(&LanguageId::Astro) {
        append_section(out, "Astro", &[".astro/"], seen);
    }
    for spec in FRAMEWORK_IGNORES {
        if ctx.frameworks.contains(&spec.key) {
            append_section(out, spec.title, spec.entries, seen);
        }
    }
    if ctx.features.contains(&FeatureId::PandaCss) {
        append_section(out, "Panda CSS", &["styled-system/"], seen);
    }
}

/// Composes a deduplicated, section-commented `.gitignore` managed block for `ctx`.
#[must_use]
pub fn compose_gitignore(ctx: &ProjectContext) -> String {
    let mut body = String::new();
    let mut seen = BTreeSet::new();

    append_section(&mut body, "OS & Editors", BASE_OS_EDITOR, &mut seen);
    append_section(
        &mut body,
        "Garde-fou & Coverage",
        BASE_GARDE_FOU_COV,
        &mut seen,
    );
    append_section(
        &mut body,
        "Secrets & Environment",
        BASE_SECRETS_ENV,
        &mut seen,
    );
    append_language_sections(&mut body, ctx, &mut seen);
    append_framework_sections(&mut body, ctx, &mut seen);

    format!("{MANAGED_START}\n{body}{MANAGED_END}\n")
}

/// Non-destructively merges a `managed` block into `existing` `.gitignore` text,
/// preserving all user lines outside `# >>> garde-fou managed >>>` .. `# <<< garde-fou managed <<<`.
#[must_use]
pub fn merge_managed_block(existing: Option<&str>, managed: &str) -> String {
    let clean_managed = managed.replace("\r\n", "\n");
    let clean_managed = clean_managed.trim();
    let Some(raw_in) = existing else {
        return format!("{clean_managed}\n");
    };
    let normalized = raw_in.replace("\r\n", "\n");
    let raw = normalized.as_str();
    if raw.trim().is_empty() {
        return format!("{clean_managed}\n");
    }

    if let Some(start_idx) = raw.find(MANAGED_START)
        && let Some(rel_end) = raw[start_idx..].find(MANAGED_END)
    {
        let end_idx = start_idx + rel_end + MANAGED_END.len();
        let prefix = &raw[..start_idx];
        let tail = &raw[end_idx..];
        let suffix = tail.strip_prefix('\n').unwrap_or(tail);
        let suffix_with_nl = if suffix.is_empty() {
            String::from("\n")
        } else if suffix.ends_with('\n') {
            format!("\n{suffix}")
        } else {
            format!("\n{suffix}\n")
        };
        return format!("{prefix}{clean_managed}{suffix_with_nl}");
    }

    let trimmed_existing = raw.trim_end_matches('\n');
    format!("{trimmed_existing}\n\n{clean_managed}\n")
}

#[rustfmt::skip]
const DOCKER_BASE_IGNORES: &[&str] = &[
    ".git", ".gitignore", ".garde-fou-cache", ".env", ".env.*", "*.pem", "*.key", "coverage", "lcov.info",
];

#[rustfmt::skip]
const DOCKER_WEB_IGNORES: &[&str] = &[
    "node_modules", "dist", ".astro", ".next", ".react-router", "styled-system", ".wrangler", ".dev.vars", "out",
];

#[rustfmt::skip]
const DOCKER_PY_IGNORES: &[&str] = &[
    ".venv", "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache",
];

/// Composes a strict `.dockerignore` satisfying `GF034` for all languages active in `ctx`.
#[must_use]
pub fn compose_dockerignore(ctx: &ProjectContext) -> String {
    let mut body = String::new();
    let mut seen = BTreeSet::new();
    append_section(
        &mut body,
        "VCS, Secrets & Local Caches",
        DOCKER_BASE_IGNORES,
        &mut seen,
    );
    if ctx.languages.contains(&LanguageId::Rust) {
        append_section(&mut body, "Rust Build Artifacts", &["target"], &mut seen);
    }
    if ctx.languages.contains(&LanguageId::TypeScript) || ctx.languages.contains(&LanguageId::Astro)
    {
        append_section(
            &mut body,
            "Node & Web Build Artifacts",
            DOCKER_WEB_IGNORES,
            &mut seen,
        );
    }
    if ctx.languages.contains(&LanguageId::Python) {
        append_section(
            &mut body,
            "Python Virtualenvs & Bytecode",
            DOCKER_PY_IGNORES,
            &mut seen,
        );
    }
    format!("{MANAGED_START}\n{body}{MANAGED_END}\n")
}
