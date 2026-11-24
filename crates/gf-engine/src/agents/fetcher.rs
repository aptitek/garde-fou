//! Hardened `skills.sh` Git shallow/pinned fetcher (`core.hooksPath=/dev/null`,
//! `protocol.file.allow=never`, zero `npx` pre-audit execution) plus `--skills-from-dir` mirror.

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
    /// Fetch live via hardened `git` shallow clone / commit pin into isolated staging.
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

fn run_hardened_git(cwd: &Path, args: &[&str], id: &str) -> Result<(), String> {
    let out = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.file.allow=never",
        ])
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git unavailable for `{id}`: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first_line = stderr.lines().next().unwrap_or("git command failed").trim();
        return Err(first_line.to_string());
    }
    Ok(())
}

fn try_git_shallow_stage(stage: &Path, spec: &ResolvedSkillSpec) -> Result<PathBuf, String> {
    let repo_url = format!("https://github.com/{}.git", spec.repo);
    let clone_dir = stage.join("repo");
    if let Some(rev) = &spec.pinned_rev {
        fs::create_dir_all(&clone_dir)
            .map_err(|e| format!("failed to prepare `{}` stage: {e}", spec.id))?;
        run_hardened_git(&clone_dir, &["init", "-q"], &spec.id)?;
        run_hardened_git(
            &clone_dir,
            &[
                "fetch",
                "--depth",
                "1",
                "--filter=blob:none",
                &repo_url,
                rev,
            ],
            &spec.id,
        )
        .map_err(|msg| format!("could not fetch `{}`#{rev} ({msg})", spec.repo))?;
        run_hardened_git(&clone_dir, &["checkout", "-q", "FETCH_HEAD"], &spec.id)?;
    } else {
        let clone_str = clone_dir.to_string_lossy().into_owned();
        run_hardened_git(
            stage,
            &[
                "clone",
                "--depth",
                "1",
                "--filter=blob:none",
                &repo_url,
                &clone_str,
            ],
            &spec.id,
        )
        .map_err(|msg| format!("could not fetch `{}` ({msg})", spec.repo))?;
    }
    locate_candidate_in_tree(&clone_dir, spec)
        .ok_or_else(|| format!("skill `{}` not found inside `{}`", spec.id, spec.repo))
}

fn audit_and_install_candidate(src_dir: &Path, dest_dir: &Path, id: &str) -> SkillFetchOutcome {
    match audit_skill_directory(src_dir, id) {
        Err(reason) => SkillFetchOutcome::Rejected(reason),
        Ok(audited) => {
            if dest_dir.exists()
                && let Err(e) = fs::remove_dir_all(dest_dir)
            {
                return SkillFetchOutcome::Unavailable(format!("cleanup error for `{id}`: {e}"));
            }
            match copy_dir_filtered(src_dir, dest_dir) {
                Ok(()) => SkillFetchOutcome::Installed(audited),
                Err(e) => SkillFetchOutcome::Unavailable(format!("copy error for `{id}`: {e}")),
            }
        }
    }
}

/// Fetches, audits, and installs `spec` into `dest_skills_root/<spec.id>/`.
#[must_use]
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
            let outcome = match try_git_shallow_stage(&stage, spec) {
                Ok(src) => audit_and_install_candidate(&src, &dest_dir, &spec.id),
                Err(msg) => SkillFetchOutcome::Unavailable(msg),
            };
            if let Err(err) = fs::remove_dir_all(&stage) {
                return SkillFetchOutcome::Unavailable(format!(
                    "failed to remove temp stage for `{}`: {err}",
                    spec.id
                ));
            }
            outcome
        }
    }
}
