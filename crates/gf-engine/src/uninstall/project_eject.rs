//! Non-destructive project detachment (`garde uninstall --project` / `garde eject`).

use std::fs;
use std::path::Path;

use gf_core::{
    is_catalog_skill_id, is_empty_generated_agents_header, strip_garde_lefthook_commands,
    strip_hash_managed_block, strip_html_managed_block,
};

use super::types::{UninstallAction, UninstallExecReport, UninstallPlan};
use crate::config_loader::load_workspace_config;
use crate::types::{DEFAULT_CONFIG_FILENAME, EngineError, LOCK_FILENAME};

fn plan_skills_cleanup(root: &Path, purge_catalog: bool, plan: &mut UninstallPlan) {
    let skills_dir = root.join(".agents").join("skills");
    if !skills_dir.is_dir() {
        return;
    }
    let Ok(rd) = fs::read_dir(&skills_dir) else {
        return;
    };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();

    for name in names {
        let rel = format!(".agents/skills/{name}");
        if name == "garde-fou" || (purge_catalog && is_catalog_skill_id(&name)) {
            plan.actions.push(UninstallAction::DeleteDir(rel));
        } else {
            plan.actions.push(UninstallAction::PreserveCustom(rel));
        }
    }
}

struct ManagedFileRule {
    rel: &'static str,
    remove_block: fn(&str) -> String,
    delete_if_empty: fn(&str) -> bool,
}

fn plan_managed_file(
    root: &Path,
    rule: &ManagedFileRule,
    plan: &mut UninstallPlan,
) -> Result<(), EngineError> {
    let path = root.join(rule.rel);
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)?;
    let cleaned = (rule.remove_block)(&raw);
    if cleaned == raw.replace("\r\n", "\n").replace('\r', "\n") {
        return Ok(());
    }
    if (rule.delete_if_empty)(&cleaned) {
        plan.actions
            .push(UninstallAction::DeleteFile(String::from(rule.rel)));
    } else {
        plan.actions
            .push(UninstallAction::StripManagedBlock(String::from(rule.rel)));
    }
    Ok(())
}

fn plan_lefthook_cleanup(root: &Path, plan: &mut UninstallPlan) -> Result<(), EngineError> {
    let path = root.join("lefthook.yml");
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)?;
    if !raw.contains("garde") {
        return Ok(());
    }
    match strip_garde_lefthook_commands(&raw) {
        None => plan
            .actions
            .push(UninstallAction::DeleteFile(String::from("lefthook.yml"))),
        Some(cleaned) if cleaned != raw.replace("\r\n", "\n") => {
            plan.actions
                .push(UninstallAction::StripManagedBlock(String::from(
                    "lefthook.yml",
                )));
        }
        Some(_) => {}
    }
    Ok(())
}

/// Builds a pure, inspectable [`UninstallPlan`] for detaching Garde-fou from `root`.
///
/// # Errors
///
/// Returns [`EngineError`] if reading managed project files fails.
pub fn plan_project_eject(
    root: &Path,
    purge_catalog_skills: bool,
) -> Result<UninstallPlan, EngineError> {
    let mut plan = UninstallPlan::default();
    let cache_dir = load_workspace_config(root)
        .map_or_else(|_| String::from(".garde-fou-cache"), |c| c.cache_dir);

    for rel in [DEFAULT_CONFIG_FILENAME, LOCK_FILENAME] {
        if root.join(rel).is_file() {
            plan.actions
                .push(UninstallAction::DeleteFile(String::from(rel)));
        }
    }
    if root.join(&cache_dir).is_dir() {
        plan.actions.push(UninstallAction::DeleteDir(cache_dir));
    }
    plan_skills_cleanup(root, purge_catalog_skills, &mut plan);
    let rules = [
        ManagedFileRule {
            rel: "AGENTS.md",
            remove_block: strip_html_managed_block,
            delete_if_empty: is_empty_generated_agents_header,
        },
        ManagedFileRule {
            rel: ".gitignore",
            remove_block: strip_hash_managed_block,
            delete_if_empty: |s| s.trim().is_empty(),
        },
        ManagedFileRule {
            rel: ".dockerignore",
            remove_block: strip_hash_managed_block,
            delete_if_empty: |s| s.trim().is_empty(),
        },
    ];
    for rule in &rules {
        plan_managed_file(root, rule, &mut plan)?;
    }
    plan_lefthook_cleanup(root, &mut plan)?;
    Ok(plan)
}

fn apply_strip_action(root: &Path, rel: &str) -> Result<(), EngineError> {
    let path = root.join(rel);
    let raw = fs::read_to_string(&path)?;
    let updated = match rel {
        "AGENTS.md" => strip_html_managed_block(&raw),
        ".gitignore" | ".dockerignore" => strip_hash_managed_block(&raw),
        "lefthook.yml" => strip_garde_lefthook_commands(&raw).unwrap_or_default(),
        _ => raw,
    };
    fs::write(&path, updated)?;
    Ok(())
}

fn cleanup_empty_agents_dirs(root: &Path) {
    let skills_dir = root.join(".agents").join("skills");
    if skills_dir.is_dir() && fs::read_dir(&skills_dir).is_ok_and(|mut r| r.next().is_none()) {
        fs::remove_dir(&skills_dir).ok();
    }
    let agents_dir = root.join(".agents");
    if agents_dir.is_dir() && fs::read_dir(&agents_dir).is_ok_and(|mut r| r.next().is_none()) {
        fs::remove_dir(&agents_dir).ok();
    }
}

/// Executes surgical project detachment in `root`, preserving all human content and custom skills.
///
/// # Errors
///
/// Returns [`EngineError`] if reading, stripping, or deleting Garde-fou files fails.
pub fn execute_project_eject(
    root: &Path,
    purge_catalog_skills: bool,
) -> Result<UninstallExecReport, EngineError> {
    let plan = plan_project_eject(root, purge_catalog_skills)?;
    let mut applied = Vec::with_capacity(plan.actions.len());
    for action in plan.actions {
        match &action {
            UninstallAction::DeleteFile(rel) => {
                let p = root.join(rel);
                if p.is_file() {
                    fs::remove_file(&p)?;
                }
            }
            UninstallAction::DeleteDir(rel) => {
                let p = root.join(rel);
                if p.is_dir() {
                    fs::remove_dir_all(&p)?;
                }
            }
            UninstallAction::StripManagedBlock(rel) => apply_strip_action(root, rel)?,
            UninstallAction::PreserveCustom(_)
            | UninstallAction::MiseUninstall(_)
            | UninstallAction::RemoveSelfBinary(_)
            | UninstallAction::NoteSecondaryBinary(_) => {}
        }
        applied.push(action);
    }
    cleanup_empty_agents_dirs(root);
    Ok(UninstallExecReport {
        applied,
        warnings: plan.warnings,
    })
}
