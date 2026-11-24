//! Installed skill indexer and pure [`AgenticSyncStatus`] verifier (`GF070`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use gf_core::{
    AgenticSyncStatus, Config, EMBEDDED_GARDE_FOU_SKILL, ProjectContext, ScannerPolicy, SkillEntry,
    compose_agents_managed_block, extract_agents_managed_block,
};

use super::skill_audit::extract_skill_summary;
use crate::types::EngineError;

pub(super) const AGENTS_MD_FILENAME: &str = "AGENTS.md";
pub(super) const SKILLS_REL_DIR: &str = ".agents/skills";
pub(super) const CORE_SKILL_ID: &str = "garde-fou";

pub(super) fn normalize_lf(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Enumerates all installed skills under `<root>/.agents/skills/*/SKILL.md` in deterministic order.
#[must_use]
pub fn collect_installed_skill_entries(root: &Path) -> Vec<SkillEntry> {
    let skills_root = root.join(SKILLS_REL_DIR);
    let Ok(entries) = fs::read_dir(skills_root) else {
        return Vec::new();
    };
    let mut map: BTreeMap<String, SkillEntry> = BTreeMap::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(raw) = fs::read_to_string(dir.join("SKILL.md")) {
            let summary = extract_skill_summary(&raw, id);
            map.insert(
                String::from(id),
                SkillEntry {
                    id: String::from(id),
                    summary,
                },
            );
        }
    }
    map.into_values().collect()
}

/// Computes the pure [`AgenticSyncStatus`] of `root` for `GF070` evaluation during `garde check`.
///
/// # Errors
///
/// Returns [`EngineError`] only on unexpected filesystem read errors.
pub fn verify_agentic_state(
    root: &Path,
    ctx: &ProjectContext,
    config: &Config,
) -> Result<AgenticSyncStatus, EngineError> {
    let policy = config.agents.sync_policy;
    if policy == ScannerPolicy::Off {
        return Ok(AgenticSyncStatus::NotApplicable);
    }

    let agents_path = root.join(AGENTS_MD_FILENAME);
    let raw_agents = agents_path
        .is_file()
        .then(|| fs::read_to_string(&agents_path))
        .transpose()?;
    let extracted = raw_agents.as_deref().and_then(extract_agents_managed_block);

    let Some(actual_block) = extracted else {
        return Ok(if policy == ScannerPolicy::Required {
            AgenticSyncStatus::MissingAgentsMd
        } else {
            AgenticSyncStatus::NotApplicable
        });
    };

    let core_skill_path = root
        .join(SKILLS_REL_DIR)
        .join(CORE_SKILL_ID)
        .join("SKILL.md");
    let Ok(raw_core_skill) = fs::read_to_string(core_skill_path) else {
        return Ok(AgenticSyncStatus::MissingOrOutdatedCoreSkill);
    };
    if normalize_lf(&raw_core_skill).trim() != normalize_lf(EMBEDDED_GARDE_FOU_SKILL).trim() {
        return Ok(AgenticSyncStatus::MissingOrOutdatedCoreSkill);
    }

    let entries = collect_installed_skill_entries(root);
    let expected_block = compose_agents_managed_block(ctx, config, &entries);
    if actual_block == expected_block {
        Ok(AgenticSyncStatus::Synced)
    } else {
        Ok(AgenticSyncStatus::DriftedAgentsMd)
    }
}
