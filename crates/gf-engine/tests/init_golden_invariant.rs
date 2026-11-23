//! Golden Invariant integration tests (`garde-fou init` -> `check == 0 violations`)
//! across Rust, TypeScript/Astro, Python/FastAPI, Docker, and idempotent `.gitignore` merges.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, FeatureId, FrameworkId, LanguageId, MANAGED_START};
use gf_engine::{
    CheckOptions, InitRequest, ScanScope, run_check, run_check_with_options, run_init,
};

static INIT_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_temp_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = INIT_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-init-golden-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn golden_invariant_rust_docker_hooks_and_task_runner() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("rust-docker")?;
    let req = InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("billing-core")),
        languages: BTreeSet::from([LanguageId::Rust]),
        features: BTreeSet::from([
            FeatureId::Docker,
            FeatureId::GitHooks,
            FeatureId::TaskRunner,
        ]),
        frameworks: BTreeSet::new(),
        run_git_init: true,
    };

    let init_report = run_init(&req)?;
    assert!(init_report.git_initialized);
    assert!(dir.join(".git").is_dir());
    for expected in [
        ".gitignore",
        ".dockerignore",
        "Dockerfile",
        "lefthook.yml",
        "Taskfile.yml",
        "Cargo.toml",
        "src/lib.rs",
        "garde-fou.toml",
        "AGENTS.md",
    ] {
        assert!(dir.join(expected).is_file(), "missing {expected}");
    }

    let direct = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    assert_eq!(direct.diagnostics, Vec::new());

    let configured = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(configured.diagnostics, Vec::new());

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn golden_invariant_typescript_astro_and_docker() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("ts-astro")?;
    let req = InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("astro-portal")),
        languages: BTreeSet::from([LanguageId::TypeScript]),
        features: BTreeSet::from([FeatureId::Docker]),
        frameworks: BTreeSet::from([FrameworkId::Astro]),
        run_git_init: true,
    };

    let init_report = run_init(&req)?;
    assert!(init_report.git_initialized);
    assert!(dir.join(".git").is_dir());
    let gitignore = fs::read_to_string(dir.join(".gitignore"))?;
    assert!(gitignore.contains("node_modules/"));
    assert!(gitignore.contains(".astro/"));
    assert!(dir.join("src/pages/index.tsx").is_file());

    let direct = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    assert_eq!(direct.diagnostics, Vec::new());

    let configured = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(configured.diagnostics, Vec::new());

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn golden_invariant_python_fastapi_docker_and_ci() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("py-fastapi")?;
    let req = InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("orders-api")),
        languages: BTreeSet::from([LanguageId::Python]),
        features: BTreeSet::from([FeatureId::Docker, FeatureId::CiGithub]),
        frameworks: BTreeSet::from([FrameworkId::FastApi]),
        run_git_init: true,
    };

    let init_report = run_init(&req)?;
    assert!(init_report.git_initialized);
    assert!(dir.join(".git").is_dir());
    let gitignore = fs::read_to_string(dir.join(".gitignore"))?;
    assert!(gitignore.contains(".venv/"));
    assert!(gitignore.contains("__pycache__/"));
    assert!(dir.join("src/orders_api/core.py").is_file());
    assert!(dir.join("app/main.py").is_file());
    assert!(dir.join(".github/workflows/garde-fou.yml").is_file());

    let direct = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    assert_eq!(direct.diagnostics, Vec::new());

    let configured = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(configured.diagnostics, Vec::new());

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn init_is_idempotent_and_preserves_custom_gitignore_and_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("idempotent")?;
    let first = run_init(&InitRequest::for_path(dir.clone()))?;
    assert!(first.git_initialized);

    let gitignore_path = dir.join(".gitignore");
    let mut current_ig = fs::read_to_string(&gitignore_path)?;
    current_ig.push_str("\n# Custom team rules\nmy-local-notes/\n");
    fs::write(&gitignore_path, &current_ig)?;

    let upgrade_req = InitRequest {
        root: dir.clone(),
        project_name: None,
        languages: BTreeSet::from([LanguageId::Python]),
        features: BTreeSet::from([FeatureId::Docker]),
        frameworks: BTreeSet::new(),
        run_git_init: true,
    };
    let second = run_init(&upgrade_req)?;
    assert!(!second.git_initialized);
    assert!(second.preserved_files.iter().any(|p| p == "src/lib.rs"));
    assert!(second.updated_files.iter().any(|p| p == ".gitignore"));

    let merged_ig = fs::read_to_string(&gitignore_path)?;
    assert!(merged_ig.contains("my-local-notes/"));
    assert!(merged_ig.contains("__pycache__/"));
    assert_eq!(merged_ig.matches(MANAGED_START).count(), 1);

    let third = run_init(&upgrade_req)?;
    assert!(third.preserved_files.iter().any(|p| p == ".gitignore"));

    let check = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    assert_eq!(check.diagnostics, Vec::new());

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
