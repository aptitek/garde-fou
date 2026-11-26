//! Golden Invariant integration tests for the 6 Proportional Arsenal archetypes
//! (`minimal`, `scripts` shell & PEP 723 python, `infra`, `knowledge`/Obsidian,
//! `academic`/Jupyter & Quarto).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{FeatureId, FrameworkId, LanguageId, WorkspaceArchetype};
use gf_engine::{CheckOptions, InitRequest, ScanScope, run_check_with_options, run_init};

static SEQ: AtomicU64 = AtomicU64::new(0);
type TestRes = Result<(), Box<dyn std::error::Error>>;

fn fresh_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let id = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-arch-{tag}-{nanos}-{id}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn check_no_cache(dir: &Path) -> Result<Vec<gf_core::Diagnostic>, Box<dyn std::error::Error>> {
    let rep = run_check_with_options(dir, &CheckOptions::new(ScanScope::All, true))?;
    Ok(rep.diagnostics)
}

#[test]
fn golden_1_minimal_zero_language_init_is_clean() -> TestRes {
    let dir = fresh_dir("minimal")?;
    let rep = run_init(&InitRequest::for_path(dir.clone()))?;
    assert_eq!(rep.context.archetype, WorkspaceArchetype::Minimal);
    assert!(rep.context.languages.is_empty());
    assert!(!dir.join("Cargo.toml").exists());
    assert!(!dir.join("package.json").exists());
    assert!(!dir.join("pyproject.toml").exists());
    assert!(!dir.join("src").exists());
    assert!(dir.join("garde-fou.toml").is_file());
    assert!(dir.join("AGENTS.md").is_file());
    assert!(dir.join("garde.lock").is_file());
    assert_eq!(check_no_cache(&dir)?, Vec::new());
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_2_scripts_shell_needs_no_tests_dir_but_catches_shell_hazards() -> TestRes {
    let dir = fresh_dir("scripts-sh")?;
    let req = InitRequest {
        languages: BTreeSet::from([LanguageId::Shell]),
        ..InitRequest::for_path(dir.clone())
    };
    let rep = run_init(&req)?;
    assert_eq!(rep.context.archetype, WorkspaceArchetype::Scripts);
    assert!(dir.join("install.sh").is_file());
    assert!(!dir.join("tests").exists());
    assert!(!dir.join("Cargo.toml").exists());
    assert_eq!(check_no_cache(&dir)?, Vec::new());

    fs::write(
        dir.join("install.sh"),
        "#!/bin/sh\nset -eu\nrm $UNQUOTED\ncurl http://example.com/a.sh | sh\n",
    )?;
    let diags = check_no_cache(&dir)?;
    assert!(diags.iter().any(|d| d.message.contains("SH-SC2086")));
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("SH-CURL-PIPE-UNSAFE"))
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_3_scripts_pep723_python_allows_print_and_validates_pep723() -> TestRes {
    let dir = fresh_dir("scripts-py")?;
    let req = InitRequest {
        archetype: Some(WorkspaceArchetype::Scripts),
        languages: BTreeSet::from([LanguageId::Python]),
        ..InitRequest::for_path(dir.clone())
    };
    let rep = run_init(&req)?;
    assert_eq!(rep.context.archetype, WorkspaceArchetype::Scripts);
    assert!(dir.join("script.py").is_file());
    assert_eq!(check_no_cache(&dir)?, Vec::new());

    // Unpinned dependency + mutable default argument must be blocked.
    fs::write(
        dir.join("script.py"),
        "# /// script\n# dependencies = [\"requests\"]\n# ///\n\ndef bad(items: list[str] = []) -> None:\n    print(items)\n",
    )?;
    let diags = check_no_cache(&dir)?;
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("PY-PEP723-UNPINNED"))
    );
    assert!(diags.iter().any(|d| d.message.contains("PY-B006")));
    assert!(!diags.iter().any(|d| d.message.contains("PY-T201")));

    // Invalid TOML inside PEP 723 block must be blocked.
    fs::write(
        dir.join("script.py"),
        "# /// script\n# dependencies = [\"unclosed\n# ///\nprint(\"ok\")\n",
    )?;
    let diags2 = check_no_cache(&dir)?;
    assert!(
        diags2
            .iter()
            .any(|d| d.message.contains("PY-PEP723-INVALID"))
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_4_infra_docker_stack_without_rust_is_clean() -> TestRes {
    let dir = fresh_dir("infra")?;
    let req = InitRequest {
        features: BTreeSet::from([FeatureId::Docker]),
        ..InitRequest::for_path(dir.clone())
    };
    let rep = run_init(&req)?;
    assert_eq!(rep.context.archetype, WorkspaceArchetype::Infra);
    assert!(dir.join("compose.yaml").is_file());
    assert!(dir.join("Dockerfile").is_file());
    assert!(dir.join(".env.example").is_file());
    assert!(!dir.join("Cargo.toml").exists());
    assert_eq!(check_no_cache(&dir)?, Vec::new());
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_5_knowledge_obsidian_vault_is_clean_and_catches_broken_wikilinks() -> TestRes {
    let dir = fresh_dir("obsidian")?;
    let req = InitRequest {
        frameworks: BTreeSet::from([FrameworkId::Obsidian]),
        ..InitRequest::for_path(dir.clone())
    };
    let rep = run_init(&req)?;
    assert_eq!(rep.context.archetype, WorkspaceArchetype::Knowledge);
    assert!(dir.join("index.md").is_file());
    assert!(dir.join(".obsidian/app.json").is_file());
    let gi = fs::read_to_string(dir.join(".gitignore"))?;
    assert!(gi.contains(".obsidian/workspace.json"));
    assert_eq!(check_no_cache(&dir)?, Vec::new());

    fs::write(
        dir.join("index.md"),
        "# Vault Index\n\nSee [[Missing Note]] for details.\n",
    )?;
    let diags = check_no_cache(&dir)?;
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("OBS-BROKEN-WIKILINK-OR-EMBED"))
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_6_and_7_academic_jupyter_and_quarto_are_clean() -> TestRes {
    let nb_dir = fresh_dir("jupyter")?;
    let nb_init = InitRequest {
        frameworks: BTreeSet::from([FrameworkId::Jupyter]),
        ..InitRequest::for_path(nb_dir.clone())
    };
    let nb_report = run_init(&nb_init)?;
    assert_eq!(nb_report.context.archetype, WorkspaceArchetype::Academic);
    assert!(nb_dir.join("notebook.ipynb").is_file());
    let nb_gi = fs::read_to_string(nb_dir.join(".gitignore"))?;
    assert!(nb_gi.contains(".ipynb_checkpoints/"));
    assert_eq!(check_no_cache(&nb_dir)?, Vec::new());
    fs::remove_dir_all(&nb_dir)?;

    let q_dir = fresh_dir("quarto")?;
    let q_init = InitRequest {
        frameworks: BTreeSet::from([FrameworkId::Quarto]),
        ..InitRequest::for_path(q_dir.clone())
    };
    let q_report = run_init(&q_init)?;
    assert_eq!(q_report.context.archetype, WorkspaceArchetype::Academic);
    assert!(q_dir.join("_quarto.yml").is_file());
    assert!(q_dir.join("index.qmd").is_file());
    let q_gi = fs::read_to_string(q_dir.join(".gitignore"))?;
    assert!(q_gi.contains("/.quarto/"));
    assert_eq!(check_no_cache(&q_dir)?, Vec::new());
    fs::remove_dir_all(&q_dir)?;
    Ok(())
}
