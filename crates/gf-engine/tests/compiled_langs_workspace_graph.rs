//! Multi-file workspace graph tests (`GF071` Tarjan, `GF072` Boundaries, `GF075` Go implicit
//! structural interfaces) and Golden Invariants (`garde init --offline --lang go|kotlin|csharp`).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{BoundaryRule, Config, LanguageId};
use gf_engine::{
    CheckOptions, InitRequest, ScanScope, run_check, run_check_with_options, run_init,
};

fn temp_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir().join(format!("gf-compiled-ws-{tag}-{nanos}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn go_structural_interfaces_satisfy_gf075_implicitly_and_flag_single_impl()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("go-iface")?;
    fs::write(dir.join("go.mod"), "module acme.io/app\n\ngo 1.24\n")?;
    fs::create_dir_all(dir.join("internal/store"))?;
    fs::create_dir_all(dir.join("internal/pg"))?;
    fs::create_dir_all(dir.join("internal/mem"))?;
    fs::write(
        dir.join("internal/store/store.go"),
        "package store\ntype Repo interface { Find(id string) error }\n",
    )?;
    fs::write(
        dir.join("internal/pg/pg.go"),
        "package pg\ntype PgRepo struct{}\nfunc (r *PgRepo) Find(id string) error { return nil }\n",
    )?;

    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let one_impl = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(one_impl.diagnostics.iter().any(|d| d.rule_id == "GF075"));

    fs::write(
        dir.join("internal/mem/mem.go"),
        "package mem\ntype MemRepo struct{}\nfunc (r *MemRepo) Find(id string) error { return nil }\n",
    )?;
    let two_impls = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(!two_impls.diagnostics.iter().any(|d| d.rule_id == "GF075"));

    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn seed_cycle_fixture(dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    for sub in [
        "internal/a",
        "internal/b",
        "src/main/kotlin/domain",
        "src/main/kotlin/infra",
        "src/Domain",
        "src/Infra",
    ] {
        fs::create_dir_all(dir.join(sub))?;
    }
    fs::write(dir.join("go.mod"), "module acme.io/app\n\ngo 1.24\n")?;
    fs::write(
        dir.join("internal/a/a.go"),
        "package a\nimport \"acme.io/app/internal/b\"\nfunc A() { b.B() }\n",
    )?;
    fs::write(
        dir.join("internal/b/b.go"),
        "package b\nimport \"acme.io/app/internal/a\"\nfunc B() { a.A() }\n",
    )?;
    fs::write(
        dir.join("src/main/kotlin/domain/D.kt"),
        "package com.acme.domain\nimport com.acme.infra.I\nfun d() = I()\n",
    )?;
    fs::write(
        dir.join("src/main/kotlin/infra/I.kt"),
        "package com.acme.infra\nimport com.acme.domain.d\nclass I\n",
    )?;
    fs::write(
        dir.join("src/Domain/D.cs"),
        "using Acme.Infra;\nnamespace Acme.Domain;\npublic class D { }\n",
    )?;
    fs::write(
        dir.join("src/Infra/I.cs"),
        "using Acme.Domain;\nnamespace Acme.Infra;\npublic class I { }\n",
    )?;
    Ok(())
}

#[test]
fn go_kotlin_and_csharp_detect_cycles_gf071_and_boundaries_gf072()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("cycles")?;
    seed_cycle_fixture(&dir)?;

    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    cfg.boundaries.push(BoundaryRule {
        from: String::from("src/Domain/**"),
        allow: Vec::new(),
        deny: vec![String::from("src/Infra/**")],
        reason: String::from("Domain must not depend on Infra"),
    });
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    let gf071_paths: Vec<&str> = rep
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF071")
        .map(|d| d.path.as_str())
        .collect();
    assert!(gf071_paths.iter().any(|p| p.contains("internal/a/")));
    assert!(gf071_paths.iter().any(|p| p.contains("src/main/kotlin/")));
    assert!(gf071_paths.iter().any(|p| p.contains("src/Domain/")));
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.rule_id == "GF072" && d.path.as_str() == "src/Domain/D.cs")
    );

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_init_for_go_kotlin_and_csharp_pass_fresh_check_with_zero_violations()
-> Result<(), Box<dyn std::error::Error>> {
    for (tag, lang) in [
        ("go", LanguageId::Go),
        ("kotlin", LanguageId::Kotlin),
        ("csharp", LanguageId::CSharp),
    ] {
        let dir = temp_ws(&format!("init-{tag}"))?;
        let mut req = InitRequest::for_path(dir.clone());
        req.project_name = Some(format!("demo-{tag}"));
        req.languages.insert(lang);
        req.run_git_init = false;
        run_init(&req)?;

        let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
        assert!(
            report.is_clean(),
            "expected 0 violations for `garde init --lang {tag}`, got: {:?}",
            report.diagnostics
        );
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
