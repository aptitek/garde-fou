//! Resilient 2-tier `skills.sh` fetcher (`npx skills add` in isolated staging + `git clone`
//! fallback without Node.js) plus `--skills-from-dir` offline mirror support.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::ResolvedSkillSpec;

use super::skill_audit::{AuditedSkill, audit_skill_directory};

static STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Source strategy for installing contextual `skills.sh` skills.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SkillsFetchMode {
    /// Fetch live from `skills.sh` (`npx` staging or `git` shallow clone fallback).
    #[default]
    Online,
    /// Zero network calls; write only embedded `garde-fou` skill and preserve existing skills.
    Offline,
    /// Install from a local filesystem directory mirror (for air-gapped CI and deterministic tests).
    FromDir(PathBuf),
}

/// Outcome of attempting to fetch, audit, and install a single skill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillFetchOutcome {
    /// Audited and installed into `.agents/skills/<id>/`.
    Installed(AuditedSkill),
    /// Rejected by supply-chain/guardrail security audit.
    Rejected(String),
    /// Network or tool unavailable (degrades gracefully to warning).
    Unavailable(String),
}

fn fresh_staging_dir(skill_id: &str) -> Result<PathBuf, std::io::Error> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seq = STAGE_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-skill-stage-{skill_id}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn is_skipped_dir_name(p: &Path) -> bool {
    p.file_name()
        .is_some_and(|n| n == ".git" || n == "node_modules")
}

fn is_matching_skill_dir(p: &Path, skill_id: &str) -> bool {
    p.file_name().is_some_and(|n| n == skill_id) && p.join("SKILL.md").is_file()
}

fn find_skill_folder_recursive(dir: &Path, skill_id: &str, depth: u8) -> Option<PathBuf> {
    if depth > 5 {
        return None;
    }
    let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !is_skipped_dir_name(p))
        .collect();
    subdirs.sort();
    if let Some(hit) = subdirs.iter().find(|p| is_matching_skill_dir(p, skill_id)) {
        return Some(hit.clone());
    }
    subdirs
        .into_iter()
        .find_map(|sub| find_skill_folder_recursive(&sub, skill_id, depth + 1))
}

fn locate_candidate_in_tree(base: &Path, spec: &ResolvedSkillSpec) -> Option<PathBuf> {
    let probes = [
        base.join(&spec.skill_path),
        base.join("skills").join(&spec.id),
        base.join(&spec.id),
        base.join(".agents/skills").join(&spec.id),
        base.join(".claude/skills").join(&spec.id),
    ];
    for cand in probes {
        if cand.join("SKILL.md").is_file() {
            return Some(cand);
        }
    }
    if let Some(found) = find_skill_folder_recursive(base, &spec.id, 0) {
        return Some(found);
    }
    base.join("SKILL.md").is_file().then(|| base.to_path_buf())
}

fn copy_dir_filtered(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(dst)?;
    for entry_res in fs::read_dir(src)? {
        let entry = entry_res?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let target = dst.join(&name);
        if path.is_dir() {
            copy_dir_filtered(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn try_npx_skills_stage(stage: &Path, spec: &ResolvedSkillSpec) -> Option<PathBuf> {
    let out = Command::new("npx")
        .args([
            "--yes", "skills", "add", &spec.repo, "--skill", &spec.id, "--yes",
        ])
        .current_dir(stage)
        .env("CI", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    locate_candidate_in_tree(stage, spec)
}

fn try_git_shallow_stage(stage: &Path, spec: &ResolvedSkillSpec) -> Result<PathBuf, String> {
    let repo_url = format!("https://github.com/{}.git", spec.repo);
    let clone_dir = stage.join("repo");
    let out = Command::new("git")
        .args(["clone", "--depth", "1", "--filter=blob:none", &repo_url])
        .arg(&clone_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git unavailable for `{}`: {e}", spec.id))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first_line = stderr.lines().next().unwrap_or("clone failed").trim();
        return Err(format!("could not fetch `{}` ({first_line})", spec.repo));
    }
    locate_candidate_in_tree(&clone_dir, spec)
        .ok_or_else(|| format!("skill `{}` not found inside `{}`", spec.id, spec.repo))
}

fn audit_and_install_candidate(src_dir: &Path, dest_dir: &Path, id: &str) -> SkillFetchOutcome {
    match audit_skill_directory(src_dir, id) {
        Err(reason) => SkillFetchOutcome::Rejected(reason),
        Ok(audited) => {
            if dest_dir.exists() {
                let _ = fs::remove_dir_all(dest_dir);
            }
            match copy_dir_filtered(src_dir, dest_dir) {
                Ok(()) => SkillFetchOutcome::Installed(audited),
                Err(e) => SkillFetchOutcome::Unavailable(format!("copy error for `{id}`: {e}")),
            }
        }
    }
}

/// Fetches, audits, and installs `spec` into `dest_skills_root/<spec.id>/`.
pub fn fetch_and_install_skill(
    spec: &ResolvedSkillSpec,
    dest_skills_root: &Path,
    mode: &SkillsFetchMode,
) -> SkillFetchOutcome {
    let dest_dir = dest_skills_root.join(&spec.id);
    match mode {
        SkillsFetchMode::Offline => {
            SkillFetchOutcome::Unavailable(String::from("offline mode active"))
        }
        SkillsFetchMode::FromDir(mirror) => {
            let repo_sub = mirror.join(&spec.repo);
            let found = locate_candidate_in_tree(mirror, spec).or_else(|| {
                repo_sub
                    .is_dir()
                    .then(|| locate_candidate_in_tree(&repo_sub, spec))
                    .flatten()
            });
            found.map_or_else(
                || {
                    SkillFetchOutcome::Unavailable(format!(
                        "skill `{}` not found in mirror `{}`",
                        spec.id,
                        mirror.display()
                    ))
                },
                |src| audit_and_install_candidate(&src, &dest_dir, &spec.id),
            )
        }
        SkillsFetchMode::Online => {
            let Ok(stage) = fresh_staging_dir(&spec.id) else {
                return SkillFetchOutcome::Unavailable(String::from("failed to create temp stage"));
            };
            let staged_res = try_npx_skills_stage(&stage, spec)
                .map_or_else(|| try_git_shallow_stage(&stage, spec), Ok);
            let outcome = match staged_res {
                Ok(src) => audit_and_install_candidate(&src, &dest_dir, &spec.id),
                Err(msg) => SkillFetchOutcome::Unavailable(msg),
            };
            let _ = fs::remove_dir_all(&stage);
            outcome
        }
    }
}
