//! Integration tests for 6-Domain Segregation (`GF060`–`GF066`) and the
//! 6-Domain `garde init -> check == 0 violations` Golden Invariant.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, FeatureId, FrameworkId, LanguageId};
use gf_engine::{
    CheckOptions, InitRequest, ScanScope, run_check, run_check_with_options, run_init,
};

static DOM_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_dom_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = DOM_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-6dom-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_rel(root: &Path, rel: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, bytes)
}

fn populate_dirty_code_and_styles(dir: &Path) -> Result<(), std::io::Error> {
    write_rel(
        dir,
        "garde-fou.toml",
        b"schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        dir,
        "package.json",
        br#"{"name":"dirty-ui","dependencies":{"react":"^19.0.0"}}"#,
    )?;
    write_rel(
        dir,
        "src/App.tsx",
        b"import { runDeploy } from \"../scripts/deploy\";\n\
          export function App() {\n\
            void runDeploy;\n\
            return (\n\
              <div style={{ color: \"#ff0000\" }} onClick={() => {}}>\n\
                <span>Cliquez ici</span>\n\
                <svg viewBox=\"0 0 10 10\"><circle cx=\"5\" cy=\"5\" r=\"4\" /></svg>\n\
                <img src=\"/assets/missing.png\" />\n\
                <button />\n\
              </div>\n\
            );\n\
          }\n",
    )?;
    write_rel(
        dir,
        "styles/card.css",
        b".card {\n  color: #123456;\n  padding: 14px !important;\n  content: \"Bonjour\";\n}\n",
    )
}

fn populate_dirty_content_locales_and_assets(dir: &Path) -> Result<(), std::io::Error> {
    write_rel(
        dir,
        "locales/fr.json",
        br#"{"nav":{"home":"<b>Accueil</b>","cta":"Partir"}}"#,
    )?;
    write_rel(dir, "locales/en.json", br#"{"nav":{"home":"Home"}}"#)?;
    write_rel(
        dir,
        "content/cours.md",
        b"# Chapitre 1\n\n<div class=\"bad\">HTML brut</div>\n\n### Saut de niveau H3\n",
    )?;
    write_rel(
        dir,
        "src/logo.png",
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
    )?;
    write_rel(
        dir,
        "assets/unused.svg",
        b"<svg onload=\"alert(1)\" xmlns=\"http://www.w3.org/2000/svg\"></svg>\n",
    )?;
    write_rel(
        dir,
        "scripts/deploy.ts",
        b"export function runDeploy(): number {\n  return 1;\n}\n",
    )?;
    write_rel(dir, "scripts/orphan.sh", b"#!/bin/sh\necho ok\n")
}

fn populate_dirty_six_domain_workspace(dir: &Path) -> Result<(), std::io::Error> {
    populate_dirty_code_and_styles(dir)?;
    populate_dirty_content_locales_and_assets(dir)
}

#[test]
fn dirty_workspace_triggers_gf060_through_gf066_simultaneously()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_dom_dir("dirty-all")?;
    populate_dirty_six_domain_workspace(&dir)?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let emitted_rules: BTreeSet<&str> = report
        .diagnostics
        .iter()
        .map(|d| d.rule_id.as_str())
        .collect();

    for expected in [
        "GF060", "GF061", "GF062", "GF063", "GF064", "GF065", "GF066",
    ] {
        assert!(
            emitted_rules.contains(expected),
            "expected rule `{expected}` in {emitted_rules:?}\ndiagnostics: {:#?}",
            report.diagnostics
        );
    }

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn golden_invariant_react_and_astro_six_domains_pass_with_zero_violations()
-> Result<(), Box<dyn std::error::Error>> {
    for (tag, fw) in [
        ("react-6d", FrameworkId::React),
        ("astro-6d", FrameworkId::Astro),
    ] {
        let dir = fresh_dom_dir(tag)?;
        let req = InitRequest {
            root: dir.clone(),
            project_name: Some(String::from("clean-portal")),
            languages: BTreeSet::from([LanguageId::TypeScript]),
            features: BTreeSet::from([FeatureId::Ui, FeatureId::I18n, FeatureId::Content]),
            frameworks: BTreeSet::from([fw]),
            run_git_init: true,
        };
        let init_rep = run_init(&req)?;
        assert!(init_rep.git_initialized);

        for expected_file in [
            "styles/tokens.css",
            "styles/app.css",
            "locales/en.json",
            "locales/fr.json",
            "content/index.md",
            "assets/logo.svg",
        ] {
            assert!(dir.join(expected_file).is_file(), "missing {expected_file}");
        }

        let direct = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
        assert_eq!(
            direct.diagnostics,
            Vec::new(),
            "direct check failed for {tag}"
        );

        let configured = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
        assert_eq!(
            configured.diagnostics,
            Vec::new(),
            "configured check failed for {tag}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}
