//! Workspace Golden Invariants for `garde init` (`mern`, `kotlin`, `python`, `typescript+nodejs`)
//! plus workspace-level `GRADLE-USE-VERSION-CATALOG` (`GF081`) and MERN `GF072` boundary tests.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, FrameworkId, LanguageId};
use gf_engine::{InitRequest, ScanScope, SkillsFetchMode, run_check, run_init_with_sync_mode};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_ws(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-batch1-{label}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn golden_init_for_mern_kotlin_python_and_nodejs_pass_fresh_check_with_zero_violations()
-> Result<(), Box<dyn std::error::Error>> {
    let cases: [(&str, &[LanguageId], &[FrameworkId]); 4] = [
        (
            "mern",
            &[LanguageId::TypeScript],
            &[
                FrameworkId::NodeJs,
                FrameworkId::Express,
                FrameworkId::Mongoose,
                FrameworkId::React,
            ],
        ),
        ("kotlin", &[LanguageId::Kotlin], &[]),
        ("python", &[LanguageId::Python], &[]),
        ("nodejs", &[LanguageId::TypeScript], &[FrameworkId::NodeJs]),
    ];
    for (name, langs, fws) in cases {
        let dir = temp_ws(name)?;
        let req = InitRequest {
            root: dir.clone(),
            project_name: Some(format!("demo-{name}")),
            archetype: None,
            languages: langs.iter().copied().collect::<BTreeSet<_>>(),
            features: BTreeSet::new(),
            frameworks: fws.iter().copied().collect::<BTreeSet<_>>(),
            run_git_init: false,
        };
        run_init_with_sync_mode(&req, SkillsFetchMode::Offline)?;
        let mut cfg = Config::strict_defaults();
        cfg.integrity.require_lock = false;
        let rep = run_check(&dir, ScanScope::All, &cfg)?;
        let diags: Vec<String> = rep
            .diagnostics
            .iter()
            .map(|d| format!("{} {} {}", d.rule_id, d.path, d.message))
            .collect();
        assert_eq!(diags, Vec::<String>::new(), "golden init failed for {name}");
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

#[test]
fn workspace_enforces_gradle_version_catalog_and_mern_frontend_backend_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("ws-rules")?;
    fs::create_dir_all(dir.join("gradle"))?;
    fs::write(
        dir.join("gradle/libs.versions.toml"),
        "[versions]\nktor = \"3.0.0\"\n",
    )?;
    fs::write(
        dir.join("build.gradle.kts"),
        "dependencies {\n    implementation(\"io.ktor:ktor-server-core:3.0.0\")\n}\n",
    )?;
    fs::create_dir_all(dir.join("apps/web/src"))?;
    fs::write(
        dir.join("package.json"),
        "{\"name\":\"mern\",\"type\":\"module\",\"packageManager\":\"pnpm@10.6.5\",\"dependencies\":{\"express\":\"^5.0.1\",\"mongoose\":\"^8.10.0\",\"react\":\"^19.0.0\"}}\n",
    )?;
    fs::write(
        dir.join("apps/web/src/Profile.tsx"),
        "import mongoose from \"mongoose\";\nexport function Profile() { return <div>{String(Boolean(mongoose))}</div>; }\n",
    )?;

    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.rule_id == "GF081" && d.message.contains("GRADLE-USE-VERSION-CATALOG"))
    );
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.rule_id == "GF072" && d.path.as_str() == "apps/web/src/Profile.tsx")
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}
