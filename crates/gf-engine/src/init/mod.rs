//! Non-destructive `uv init`-style project bootstrapper (`garde-fou init`).

mod templates_infra;
mod templates_python;
mod templates_rust;
mod templates_web;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gf_core::{
    Config, FeatureId, FrameworkId, LanguageId, ProjectContext, compose_dockerignore,
    compose_gitignore, merge_managed_block,
};

use crate::{
    DEFAULT_CONFIG_FILENAME, EngineError, ScanScope, detect_project_context, discover_files,
};

/// Declarative parameters for [`run_init`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitRequest {
    /// Target workspace directory (created if missing).
    pub root: PathBuf,
    /// Optional explicit project name override (defaults to directory name).
    pub project_name: Option<String>,
    /// Languages requested on the command line.
    pub languages: BTreeSet<LanguageId>,
    /// Infrastructure features requested on the command line.
    pub features: BTreeSet<FeatureId>,
    /// Application/workspace frameworks requested on the command line.
    pub frameworks: BTreeSet<FrameworkId>,
    /// Whether `git init -b main` should run when `.git` is absent.
    pub run_git_init: bool,
}

impl InitRequest {
    /// Builds a default [`InitRequest`] for `root` with `git init` enabled.
    #[must_use]
    pub const fn for_path(root: PathBuf) -> Self {
        Self {
            root,
            project_name: None,
            languages: BTreeSet::new(),
            features: BTreeSet::new(),
            frameworks: BTreeSet::new(),
            run_git_init: true,
        }
    }
}

/// Summary of files created, updated, or preserved by [`run_init`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitReport {
    /// Initialized workspace directory.
    pub root: PathBuf,
    /// Normalized kebab-case project name.
    pub project_name: String,
    /// Resolved project context (`languages` × `features` × `frameworks`).
    pub context: ProjectContext,
    /// `true` when `git init -b main` initialized a new `.git/` repository.
    pub git_initialized: bool,
    /// Relative paths created from scratch.
    pub created_files: Vec<String>,
    /// Relative paths updated via managed-block merge (e.g. `.gitignore`).
    pub updated_files: Vec<String>,
    /// Relative paths left untouched because they already existed.
    pub preserved_files: Vec<String>,
}

fn resolve_names(req: &InitRequest) -> (String, String) {
    let raw_name = req
        .project_name
        .as_deref()
        .or_else(|| req.root.file_name().and_then(|s| s.to_str()))
        .unwrap_or("app");
    templates_infra::normalize_project_names(raw_name)
}

fn infer_languages_from_frameworks(ctx: &mut ProjectContext) {
    for fw in &ctx.frameworks {
        let lang = match fw {
            FrameworkId::CargoWorkspace => LanguageId::Rust,
            FrameworkId::React | FrameworkId::Astro | FrameworkId::NextJs => LanguageId::TypeScript,
            FrameworkId::FastApi | FrameworkId::Django => LanguageId::Python,
        };
        ctx.languages.insert(lang);
    }
}

fn resolve_init_context(req: &InitRequest) -> Result<ProjectContext, EngineError> {
    let base_cfg = Config::strict_defaults();
    let existing_files = discover_files(&req.root, ScanScope::All, &base_cfg)?;
    let mut ctx = detect_project_context(&req.root, &existing_files, &base_cfg);

    ctx.languages.extend(req.languages.iter().copied());
    ctx.features.extend(req.features.iter().copied());
    ctx.frameworks.extend(req.frameworks.iter().copied());
    infer_languages_from_frameworks(&mut ctx);

    if ctx.languages.is_empty() {
        ctx.languages.insert(LanguageId::Rust);
        ctx.features.insert(FeatureId::GitHooks);
    }
    if ctx.languages.contains(&LanguageId::Rust) {
        ctx.frameworks.insert(FrameworkId::CargoWorkspace);
    }
    Ok(ctx)
}

fn ensure_git_repo(root: &Path, run_git_init: bool) -> Result<bool, EngineError> {
    if !run_git_init || root.join(".git").exists() {
        return Ok(false);
    }
    let out = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(EngineError::GitCommand(stderr.trim().to_string()));
    }
    Ok(true)
}

fn upsert_managed_file(
    root: &Path,
    rel: &str,
    managed: &str,
    report: &mut InitReport,
) -> Result<(), EngineError> {
    let target = root.join(rel);
    let existing = target
        .is_file()
        .then(|| fs::read_to_string(&target))
        .transpose()?;
    let merged = merge_managed_block(existing.as_deref(), managed);

    match existing {
        None => {
            fs::write(&target, merged)?;
            report.created_files.push(String::from(rel));
        }
        Some(prev) if prev != merged => {
            fs::write(&target, merged)?;
            report.updated_files.push(String::from(rel));
        }
        Some(_) => {
            report.preserved_files.push(String::from(rel));
        }
    }
    Ok(())
}

fn write_missing_files(
    root: &Path,
    files: Vec<(String, String)>,
    report: &mut InitReport,
) -> Result<(), EngineError> {
    for (rel, content) in files {
        let full = root.join(&rel);
        if full.exists() {
            report.preserved_files.push(rel);
            continue;
        }
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full, content)?;
        report.created_files.push(rel);
    }
    Ok(())
}

fn collect_candidate_files(
    project_name: &str,
    pkg_snake: &str,
    context: &ProjectContext,
) -> Vec<(String, String)> {
    let mut files = vec![
        (
            String::from(".gitattributes"),
            String::from(templates_infra::GITATTRIBUTES_TEMPLATE),
        ),
        (
            String::from(DEFAULT_CONFIG_FILENAME),
            templates_infra::render_garde_fou_toml(project_name, context),
        ),
        (
            String::from("AGENTS.md"),
            templates_infra::render_agents_md(project_name),
        ),
    ];
    if context.languages.contains(&LanguageId::Rust) {
        files.extend(templates_rust::rust_files(project_name, pkg_snake));
    }
    if context.languages.contains(&LanguageId::TypeScript) {
        files.extend(templates_web::web_files(project_name, context));
    }
    if context.languages.contains(&LanguageId::Python) {
        files.extend(templates_python::python_files(
            project_name,
            pkg_snake,
            context,
        ));
    }
    files.extend(templates_infra::infra_feature_files(project_name, context));
    files
}

/// Bootstraps or non-destructively adopts a project workspace in `req.root`.
///
/// # Errors
///
/// Returns [`EngineError`] if directory creation, `git init`, or file writing fails.
pub fn run_init(req: &InitRequest) -> Result<InitReport, EngineError> {
    fs::create_dir_all(&req.root)?;
    let (project_name, pkg_snake) = resolve_names(req);
    let context = resolve_init_context(req)?;
    let git_initialized = ensure_git_repo(&req.root, req.run_git_init)?;

    let mut report = InitReport {
        root: req.root.clone(),
        project_name: project_name.clone(),
        context: context.clone(),
        git_initialized,
        created_files: Vec::new(),
        updated_files: Vec::new(),
        preserved_files: Vec::new(),
    };

    upsert_managed_file(
        &req.root,
        ".gitignore",
        &compose_gitignore(&context),
        &mut report,
    )?;
    if context.features.contains(&FeatureId::Docker) {
        let di = compose_dockerignore(&context);
        upsert_managed_file(&req.root, ".dockerignore", &di, &mut report)?;
    }

    let candidates = collect_candidate_files(&project_name, &pkg_snake, &context);
    write_missing_files(&req.root, candidates, &mut report)?;
    Ok(report)
}
