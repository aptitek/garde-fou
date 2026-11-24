//! Agentic synchronization engine (`garde sync` & `GF070` verification).
//!
//! Assembles `AGENTS.md` by fragments, manages `.agents/skills/garde-fou/SKILL.md`,
//! installs audited `skills.sh` skills, and keeps `garde.lock` synchronized.

pub mod fetcher;
pub mod skill_audit;
mod verifier;

use std::fs;
use std::path::Path;

use gf_core::{
    Config, EMBEDDED_GARDE_FOU_SKILL, ProjectContext, compose_agents_managed_block,
    merge_agents_md, select_contextual_skills,
};

use crate::config_loader::load_workspace_config;
use crate::context_detector::detect_project_context;
use crate::discovery::discover_files;
use crate::lockfile::write_garde_lock;
use crate::types::{DEFAULT_CONFIG_FILENAME, EngineError, LOCK_FILENAME, ScanScope};
use fetcher::{SkillFetchOutcome, SkillsFetchMode, fetch_and_install_skill};
use verifier::{AGENTS_MD_FILENAME, CORE_SKILL_ID, SKILLS_REL_DIR, normalize_lf};
pub use verifier::{collect_installed_skill_entries, verify_agentic_state};

/// Mutation performed on `AGENTS.md` during [`run_sync`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileSyncAction {
    /// `AGENTS.md` was created from scratch.
    Created,
    /// `AGENTS.md` managed block was updated in place.
    Updated,
    /// `AGENTS.md` was already 100% up to date.
    Unchanged,
}

/// Execution options for [`run_sync`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncOptions {
    /// Network or local-mirror mode for `skills.sh` skill installation.
    pub fetch_mode: SkillsFetchMode,
    /// Re-fetch and overwrite already installed remote skills.
    pub update_skills: bool,
}

/// Structured summary returned by [`run_sync`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncReport {
    /// Resolved workspace context used to compose fragments.
    pub context: ProjectContext,
    /// Action taken on `AGENTS.md`.
    pub agents_md: FileSyncAction,
    /// Skill IDs newly installed or refreshed in `.agents/skills/`.
    pub installed_skills: Vec<String>,
    /// Existing skill IDs in `.agents/skills/` left untouched.
    pub preserved_skills: Vec<String>,
    /// `(skill_id, reason)` pairs rejected by `skill_audit`.
    pub rejected_skills: Vec<(String, String)>,
    /// Non-fatal network/mirror warnings encountered during skill fetching.
    pub warnings: Vec<String>,
    /// Whether `garde.lock` was refreshed alongside `AGENTS.md`.
    pub lock_updated: bool,
}

struct RemoteSkillsOutcome {
    installed: Vec<String>,
    preserved: Vec<String>,
    rejected: Vec<(String, String)>,
    warnings: Vec<String>,
}

fn ensure_embedded_core_skill(root: &Path) -> Result<bool, std::io::Error> {
    let dir = root.join(SKILLS_REL_DIR).join(CORE_SKILL_ID);
    fs::create_dir_all(&dir)?;
    let target = dir.join("SKILL.md");
    let expected = normalize_lf(EMBEDDED_GARDE_FOU_SKILL);
    if let Ok(existing) = fs::read_to_string(&target)
        && normalize_lf(&existing) == expected
    {
        return Ok(false);
    }
    fs::write(target, expected)?;
    Ok(true)
}

fn record_skill_outcome(id: String, outcome: SkillFetchOutcome, out: &mut RemoteSkillsOutcome) {
    match outcome {
        SkillFetchOutcome::Installed(_) => {
            out.preserved.retain(|p| p != &id);
            out.installed.push(id);
        }
        SkillFetchOutcome::Rejected(reason) => out.rejected.push((id, reason)),
        SkillFetchOutcome::Unavailable(msg) => out.warnings.push(format!("{id}: {msg}")),
    }
}

fn sync_remote_skills(
    root: &Path,
    ctx: &ProjectContext,
    config: &Config,
    options: &SyncOptions,
) -> RemoteSkillsOutcome {
    let skills_root = root.join(SKILLS_REL_DIR);
    let preserved = collect_installed_skill_entries(root)
        .into_iter()
        .map(|e| e.id)
        .filter(|id| id != CORE_SKILL_ID)
        .collect();
    let mut out = RemoteSkillsOutcome {
        installed: Vec::new(),
        preserved,
        rejected: Vec::new(),
        warnings: Vec::new(),
    };
    if !config.agents.remote_skills || options.fetch_mode == SkillsFetchMode::Offline {
        return out;
    }

    for spec in select_contextual_skills(ctx, &config.agents) {
        if skills_root.join(&spec.id).join("SKILL.md").is_file() && !options.update_skills {
            continue;
        }
        let res = fetch_and_install_skill(&spec, &skills_root, &options.fetch_mode);
        record_skill_outcome(spec.id, res, &mut out);
    }
    out
}

fn write_merged_agents_md(root: &Path, managed_block: &str) -> Result<FileSyncAction, EngineError> {
    let path = root.join(AGENTS_MD_FILENAME);
    let existing = path
        .is_file()
        .then(|| fs::read_to_string(&path))
        .transpose()?;
    let merged = merge_agents_md(existing.as_deref(), managed_block);
    match existing {
        None => {
            fs::write(&path, merged)?;
            Ok(FileSyncAction::Created)
        }
        Some(prev) if normalize_lf(&prev) != merged => {
            fs::write(&path, merged)?;
            Ok(FileSyncAction::Updated)
        }
        Some(_) => Ok(FileSyncAction::Unchanged),
    }
}

/// Synchronizes `.agents/skills/`, `AGENTS.md`, and `garde.lock` (if active) in `root`.
///
/// # Errors
///
/// Returns [`EngineError`] if configuration loading, file discovery, or disk writes fail.
pub fn run_sync(root: &Path, options: &SyncOptions) -> Result<SyncReport, EngineError> {
    let loaded = load_workspace_config(root)?;
    let config = &loaded.analyzer;
    let files = discover_files(root, ScanScope::All, config)?;
    let context = detect_project_context(root, &files, config);

    let core_written = ensure_embedded_core_skill(root)?;
    let mut skills_out = sync_remote_skills(root, &context, config, options);
    if core_written {
        skills_out.installed.insert(0, String::from(CORE_SKILL_ID));
    }

    let entries = collect_installed_skill_entries(root);
    let managed = compose_agents_managed_block(&context, config, &entries);
    let agents_md = write_merged_agents_md(root, &managed)?;

    let should_lock = (root.join(LOCK_FILENAME).is_file() || config.integrity.require_lock)
        && root.join(DEFAULT_CONFIG_FILENAME).is_file();
    if should_lock {
        let _ = write_garde_lock(root)?;
    }

    Ok(SyncReport {
        context,
        agents_md,
        installed_skills: skills_out.installed,
        preserved_skills: skills_out.preserved,
        rejected_skills: skills_out.rejected,
        warnings: skills_out.warnings,
        lock_updated: should_lock,
    })
}
