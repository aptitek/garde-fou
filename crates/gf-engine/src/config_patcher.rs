//! Lossless `toml_edit` patcher for `garde-fou.toml` preserving user comments,
//! `[limits]`, `[paths]`, and `[[boundaries]]`, followed by automatic `run_sync`.

use std::fs;
use std::path::Path;

use gf_core::{
    FeatureId, GovernanceFlag, ProjectSettingsState, compose_dockerignore, compose_gitignore,
    merge_managed_block,
};
use toml_edit::{Array, DocumentMut, Item, Table, Value, value};

use crate::agents::{SyncOptions, SyncReport, fetcher::SkillsFetchMode, run_sync};
use crate::config_loader::load_workspace_config;
use crate::context_detector::detect_project_context;
use crate::discovery::discover_files;
use crate::types::{DEFAULT_CONFIG_FILENAME, EngineError, ScanScope};

fn with_table(doc: &mut DocumentMut, name: &str, f: impl FnOnce(&mut Table)) {
    if !doc.contains_key(name) || !doc[name].is_table() {
        doc[name] = Item::Table(Table::new());
    }
    if let Some(tbl) = doc[name].as_table_mut() {
        f(tbl);
    }
}

fn string_array<'a>(items: impl Iterator<Item = &'a str>) -> Value {
    let mut arr = Array::new();
    for item in items {
        arr.push(item);
    }
    Value::Array(arr)
}

fn set_or_remove_array<'a>(table: &mut Table, key: &str, items: impl Iterator<Item = &'a str>) {
    let vals: Vec<&str> = items.collect();
    if vals.is_empty() {
        table.remove(key);
    } else {
        table[key] = value(string_array(vals.into_iter()));
    }
}

/// Loads current [`ProjectSettingsState`] combining `garde-fou.toml` with disk auto-detection.
///
/// # Errors
///
/// Returns [`EngineError`] if reading `garde-fou.toml` or discovering workspace files fails.
pub fn load_project_settings(root: &Path) -> Result<ProjectSettingsState, EngineError> {
    let loaded = load_workspace_config(root)?;
    let files = discover_files(root, ScanScope::All, &loaded.analyzer)?;
    let mut disk_only_cfg = loaded.analyzer.clone();
    disk_only_cfg.project.languages.clear();
    disk_only_cfg.project.features.clear();
    disk_only_cfg.project.frameworks.clear();
    disk_only_cfg.project.archetype = gf_core::ArchetypeSetting::Auto;
    let detected = detect_project_context(root, &files, &disk_only_cfg);
    Ok(ProjectSettingsState::from_config_and_detected(
        &loaded.analyzer,
        &detected,
    ))
}

fn patch_governance_tables(doc: &mut DocumentMut, settings: &ProjectSettingsState) {
    let gov = settings.governance;
    with_table(doc, "integrity", |integ| {
        integ["require_lock"] = value(gov.is_set(GovernanceFlag::RequireLock));
    });
    with_table(doc, "agents", |agents| {
        let sync_pol = if gov.is_set(GovernanceFlag::SyncRequired) {
            "required"
        } else {
            "auto"
        };
        agents["sync_policy"] = value(sync_pol);
        agents["remote_skills"] = value(gov.is_set(GovernanceFlag::RemoteSkills));
    });
    with_table(doc, "clean_code", |clean| {
        clean["forbid_todos"] = value(gov.is_set(GovernanceFlag::ForbidTodos));
    });
    with_table(doc, "tdd", |tdd| {
        tdd["forbid_assertion_free_tests"] =
            value(gov.is_set(GovernanceFlag::RequireRealAssertions));
    });
}

/// Losslessly patches raw `garde-fou.toml` text with `settings`, preserving all comments
/// and unrelated sections (`[limits]`, `[paths]`, `[[boundaries]]`, etc.).
///
/// # Errors
///
/// Returns [`EngineError::InvalidConfig`] if `existing` is not valid TOML.
pub fn patch_garde_fou_toml_str(
    existing: &str,
    settings: &ProjectSettingsState,
) -> Result<String, EngineError> {
    let mut doc: DocumentMut = existing
        .parse()
        .map_err(|err| EngineError::InvalidConfig(format!("toml_edit parse error: {err}")))?;
    if !doc.contains_key("schema_version") {
        doc["schema_version"] = value(1);
    }
    with_table(&mut doc, "project", |proj| {
        proj["archetype"] = value(settings.archetype.as_str());
        set_or_remove_array(
            proj,
            "languages",
            settings.languages.iter().map(|l| l.as_str()),
        );
        set_or_remove_array(
            proj,
            "features",
            settings.features.iter().map(|f| f.as_str()),
        );
        set_or_remove_array(
            proj,
            "frameworks",
            settings.frameworks.iter().map(|fw| fw.as_str()),
        );
    });
    patch_governance_tables(&mut doc, settings);

    let mut rendered = doc.to_string();
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn upsert_ignore_file(root: &Path, filename: &str, managed: &str) -> Result<(), EngineError> {
    let path = root.join(filename);
    let existing = path
        .is_file()
        .then(|| fs::read_to_string(&path))
        .transpose()?;
    let merged = merge_managed_block(existing.as_deref(), managed);
    if existing.as_deref() != Some(merged.as_str()) {
        fs::write(&path, merged)?;
    }
    Ok(())
}

/// Persists `settings` losslessly into `garde-fou.toml`, refreshes `.gitignore`/`.dockerignore`,
/// and runs [`run_sync`] (`AGENTS.md`, skills, and `garde.lock`).
///
/// # Errors
///
/// Returns [`EngineError`] if reading, patching, writing, or synchronizing fails.
pub fn apply_project_settings(
    root: &Path,
    settings: &ProjectSettingsState,
    fetch_mode: SkillsFetchMode,
) -> Result<SyncReport, EngineError> {
    let manifest_path = root.join(DEFAULT_CONFIG_FILENAME);
    let existing = if manifest_path.is_file() {
        fs::read_to_string(&manifest_path)?
    } else {
        String::from("schema_version = 1\n")
    };
    let patched = patch_garde_fou_toml_str(&existing, settings)?;
    fs::write(&manifest_path, patched)?;

    let ctx = settings.effective_context();
    upsert_ignore_file(root, ".gitignore", &compose_gitignore(&ctx))?;
    if ctx.features.contains(&FeatureId::Docker) {
        upsert_ignore_file(root, ".dockerignore", &compose_dockerignore(&ctx))?;
    }

    let options = SyncOptions {
        fetch_mode,
        update_skills: false,
    };
    run_sync(root, &options)
}
