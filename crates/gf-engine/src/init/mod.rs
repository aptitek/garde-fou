//! Non-destructive `uv init`-style project bootstrapper (`garde-fou init`).

mod framework_deps;
mod templates_archetypes;
mod templates_compiled;
mod templates_domains;
mod templates_infra;
mod templates_mern;
mod templates_python;
mod templates_rust;
mod templates_web;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gf_core::{
    Config, FeatureId, FrameworkId, LanguageId, ProjectContext, WorkspaceArchetype,
    compose_dockerignore, compose_gitignore, merge_managed_block,
};

use crate::agents::{FileSyncAction, SyncOptions, fetcher::SkillsFetchMode, run_sync};
use crate::context_detector::detect_project_context;
use crate::discovery::discover_files;
use crate::types::{EngineError, LOCK_FILENAME, ScanScope};

/// Declarative parameters for [`run_init`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitRequest {
    /// Target workspace directory (created if missing).
    pub root: PathBuf,
    /// Optional explicit project name override (defaults to directory name).
    pub project_name: Option<String>,
    /// Optional explicit workspace archetype override (`--archetype`).
    pub archetype: Option<WorkspaceArchetype>,
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
            archetype: None,
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
    /// Skill IDs installed into `.agents/skills/` during initialization.
    pub installed_skills: Vec<String>,
    /// Non-fatal warnings (e.g. offline fallback when `skills.sh` is unreachable).
    pub warnings: Vec<String>,
}

fn resolve_names(req: &InitRequest) -> (String, String) {
    let raw_name = req
        .project_name
        .as_deref()
        .or_else(|| req.root.file_name().and_then(|s| s.to_str()))
        .unwrap_or("app");
    templates_infra::normalize_project_names(raw_name)
}

fn resolve_init_context(req: &InitRequest) -> Result<ProjectContext, EngineError> {
    let base_cfg = Config::strict_defaults();
    let existing_files = discover_files(&req.root, ScanScope::All, &base_cfg)?;
    let mut ctx = detect_project_context(&req.root, &existing_files, &base_cfg);

    ctx.languages.extend(req.languages.iter().copied());
    ctx.features.extend(req.features.iter().copied());
    ctx.frameworks.extend(req.frameworks.iter().copied());
    templates_domains::finalize_init_context(&mut ctx, req.archetype);
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
        Some(_) => report.preserved_files.push(String::from(rel)),
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

fn finalize_init_sync(
    root: &Path,
    fetch_mode: SkillsFetchMode,
    lock_existed: bool,
    report: &mut InitReport,
) -> Result<(), EngineError> {
    let sync_rep = run_sync(
        root,
        &SyncOptions {
            fetch_mode,
            update_skills: false,
        },
    )?;
    match sync_rep.agents_md {
        FileSyncAction::Created => report.created_files.push(String::from("AGENTS.md")),
        FileSyncAction::Updated => report.updated_files.push(String::from("AGENTS.md")),
        FileSyncAction::Unchanged => report.preserved_files.push(String::from("AGENTS.md")),
    }
    if !lock_existed && sync_rep.lock_updated {
        report.created_files.push(String::from(LOCK_FILENAME));
    }
    report.installed_skills = sync_rep.installed_skills;
    report.warnings = sync_rep.warnings;
    Ok(())
}

/// Bootstraps or non-destructively adopts a project workspace in `req.root` (offline skills mode).
///
/// # Errors
///
/// Returns [`EngineError`] if directory creation, `git init`, or file writing fails.
pub fn run_init(req: &InitRequest) -> Result<InitReport, EngineError> {
    run_init_with_sync_mode(req, SkillsFetchMode::Offline)
}

/// Bootstraps a workspace in `req.root` using an explicit [`SkillsFetchMode`].
///
/// # Errors
///
/// Returns [`EngineError`] if directory creation, `git init`, or file writing fails.
pub fn run_init_with_sync_mode(
    req: &InitRequest,
    fetch_mode: SkillsFetchMode,
) -> Result<InitReport, EngineError> {
    fs::create_dir_all(&req.root)?;
    let (project_name, pkg_snake) = resolve_names(req);
    let context = resolve_init_context(req)?;
    let git_initialized = ensure_git_repo(&req.root, req.run_git_init)?;
    let lock_existed = req.root.join(LOCK_FILENAME).exists();

    let mut report = InitReport {
        root: req.root.clone(),
        project_name: project_name.clone(),
        context: context.clone(),
        git_initialized,
        created_files: Vec::new(),
        updated_files: Vec::new(),
        preserved_files: Vec::new(),
        installed_skills: Vec::new(),
        warnings: Vec::new(),
    };
    upsert_managed_file(
        &req.root,
        ".gitignore",
        &compose_gitignore(&context),
        &mut report,
    )?;
    if context.features.contains(&FeatureId::Docker) {
        upsert_managed_file(
            &req.root,
            ".dockerignore",
            &compose_dockerignore(&context),
            &mut report,
        )?;
    }
    let candidates = templates_domains::collect_all_init_files(&project_name, &pkg_snake, &context);
    write_missing_files(&req.root, candidates, &mut report)?;
    finalize_init_sync(&req.root, fetch_mode, lock_existed, &mut report)?;
    Ok(report)
}
