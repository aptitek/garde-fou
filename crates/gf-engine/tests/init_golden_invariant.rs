//! Golden Invariant integration tests (`garde-fou init` -> `check == 0 violations`)
//! across Rust, TypeScript/Astro, Python/FastAPI, Docker, and idempotent `.gitignore` merges.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, FeatureId, FrameworkId, LanguageId, MANAGED_START};
use gf_engine::{
    CheckOptions, CheckReport, InitRequest, ScanScope, run_check, run_check_with_options, run_init,
};

static INIT_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_temp_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = INIT_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-init-golden-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn verify_golden_init_workspace(
    req: &InitRequest,
    expected_files: &[&str],
    expected_ignores: &[&str],
) -> Result<CheckReport, Box<dyn std::error::Error>> {
    let init_report = run_init(req)?;
    assert!(init_report.git_initialized);
    assert!(req.root.join(".git").is_dir());

    for rel in expected_files {
        assert!(req.root.join(rel).is_file(), "missing {rel}");
    }
    let gitignore = fs::read_to_string(req.root.join(".gitignore"))?;
    for ig in expected_ignores {
        assert!(gitignore.contains(ig), "missing ignore `{ig}`");
    }

    let direct = run_check(&req.root, ScanScope::All, &Config::strict_defaults())?;
    assert_eq!(direct.diagnostics, Vec::new());

    let configured = run_check_with_options(&req.root, &CheckOptions::new(ScanScope::All, true))?;
    fs::remove_dir_all(&req.root)?;
    Ok(configured)
}

#[test]
fn golden_invariant_rust_stack() -> Result<(), Box<dyn std::error::Error>> {
    let rust_dir = fresh_temp_dir("rust-docker")?;
    let report = verify_golden_init_workspace(
        &InitRequest {
            root: rust_dir,
            project_name: Some(String::from("billing-core")),
            archetype: None,
            languages: BTreeSet::from([LanguageId::Rust]),
            features: BTreeSet::from([
                FeatureId::Docker,
                FeatureId::GitHooks,
                FeatureId::TaskRunner,
            ]),
            frameworks: BTreeSet::new(),
            run_git_init: true,
        },
        &[
            ".gitignore",
            ".dockerignore",
            "Dockerfile",
            "lefthook.yml",
            "Taskfile.yml",
            "Cargo.toml",
            "src/lib.rs",
            "garde-fou.toml",
            "AGENTS.md",
        ],
        &["/target/"],
    )?;
    assert_eq!(report.diagnostics, Vec::new());
    Ok(())
}

#[test]
fn golden_invariant_astro_and_python_stacks() -> Result<(), Box<dyn std::error::Error>> {
    let ts_dir = fresh_temp_dir("ts-astro")?;
    let ts_rep = verify_golden_init_workspace(
        &InitRequest {
            root: ts_dir,
            project_name: Some(String::from("astro-portal")),
            archetype: None,
            languages: BTreeSet::from([LanguageId::TypeScript]),
            features: BTreeSet::from([FeatureId::Docker]),
            frameworks: BTreeSet::from([FrameworkId::Astro]),
            run_git_init: true,
        },
        &["package.json", "tests/index.test.ts", "src/pages/index.tsx"],
        &["node_modules/", ".astro/"],
    )?;
    assert_eq!(ts_rep.diagnostics, Vec::new());

    let py_dir = fresh_temp_dir("py-fastapi")?;
    let py_rep = verify_golden_init_workspace(
        &InitRequest {
            root: py_dir,
            project_name: Some(String::from("orders-api")),
            archetype: None,
            languages: BTreeSet::from([LanguageId::Python]),
            features: BTreeSet::from([FeatureId::Docker, FeatureId::CiGithub]),
            frameworks: BTreeSet::from([FrameworkId::FastApi]),
            run_git_init: true,
        },
        &[
            "src/orders_api/core.py",
            "app/main.py",
            ".github/workflows/garde-fou.yml",
        ],
        &[".venv/", "__pycache__/"],
    )?;
    assert_eq!(py_rep.diagnostics, Vec::new());
    Ok(())
}

#[test]
fn init_is_idempotent_and_preserves_custom_gitignore_and_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("idempotent")?;
    let first_req = InitRequest {
        languages: BTreeSet::from([LanguageId::Rust]),
        ..InitRequest::for_path(dir.clone())
    };
    let first = run_init(&first_req)?;
    assert!(first.git_initialized);

    let gitignore_path = dir.join(".gitignore");
    let mut current_ig = fs::read_to_string(&gitignore_path)?;
    current_ig.push_str("\n# Custom team rules\nmy-local-notes/\n");
    fs::write(&gitignore_path, &current_ig)?;

    let upgrade_req = InitRequest {
        root: dir.clone(),
        project_name: None,
        archetype: None,
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

    fs::remove_dir_all(&dir)?;
    Ok(())
}
