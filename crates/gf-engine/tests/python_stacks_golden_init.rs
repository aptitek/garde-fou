//! Golden Invariants for `garde init --lang python --framework django|nicegui|pytorch,langchain`
//! and auto-generated Django migration (`**/migrations/0*.py`) exemption verification.

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
    let dir = std::env::temp_dir().join(format!("gf-batch2-{label}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn golden_init_for_django_nicegui_and_pytorch_langchain_pass_with_zero_violations()
-> Result<(), Box<dyn std::error::Error>> {
    let cases: [(&str, &[FrameworkId]); 3] = [
        ("django", &[FrameworkId::Django]),
        ("nicegui", &[FrameworkId::NiceGui]),
        ("ai", &[FrameworkId::PyTorch, FrameworkId::LangChain]),
    ];
    for (name, fws) in cases {
        let dir = temp_ws(name)?;
        let req = InitRequest {
            root: dir.clone(),
            project_name: Some(format!("demo-{name}")),
            archetype: None,
            languages: BTreeSet::from([LanguageId::Python]),
            features: BTreeSet::new(),
            frameworks: fws.iter().copied().collect(),
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
fn django_migration_of_260_lines_is_exempt_from_gf001_gf021_gf076()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("dj-mig")?;
    fs::create_dir_all(dir.join("src/blog/migrations"))?;
    let mut mig = String::from(
        "from django.db import migrations\nclass Migration(migrations.Migration):\n    initial = True\n    operations = []\n",
    );
    for i in 0..260 {
        use std::fmt::Write;
        let _ = writeln!(mig, "# auto-generated line {i}");
    }
    fs::write(dir.join("src/blog/migrations/0001_initial.py"), mig)?;
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(
        !rep.diagnostics
            .iter()
            .any(|d| matches!(d.rule_id.as_str(), "GF001" | "GF021" | "GF076"))
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}
