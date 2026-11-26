//! Workspace-level integration tests for PostgreSQL cross-migration RLS parity,
//! Oracle PL/SQL `PACKAGE`/`PACKAGE BODY` parity, `utPLSQL`/`pgTAP` `GF076`, and 3 DB golden inits.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Config, FrameworkId, LanguageId};
use gf_engine::{InitRequest, ScanScope, SkillsFetchMode, run_check, run_init_with_sync_mode};

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-db-ws-{tag}-{}-{n}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn postgres_cross_migration_rls_and_policy_parity() -> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("pg-rls")?;
    fs::create_dir_all(dir.join("migrations"))?;
    fs::write(
        dir.join("migrations/0001_init.sql"),
        "-- garde-dialect: postgres\nCREATE TABLE tenants_docs (id BIGINT PRIMARY KEY);\nCREATE POLICY tenant_isolation ON tenants_docs USING (true);\nCREATE TABLE audit_secrets (id BIGINT PRIMARY KEY);\nALTER TABLE audit_secrets ENABLE ROW LEVEL SECURITY;\n",
    )?;
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.message.contains("PG-POLICY-WITHOUT-RLS"))
    );
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.message.contains("PG-RLS-WITHOUT-POLICY"))
    );

    fs::write(
        dir.join("migrations/0002_enable_rls.sql"),
        "-- garde-dialect: postgres\nALTER TABLE tenants_docs ENABLE ROW LEVEL SECURITY;\nCREATE POLICY audit_read ON audit_secrets USING (true);\n",
    )?;
    let clean = run_check(&dir, ScanScope::All, &cfg)?;
    let rls_diags: Vec<_> = clean
        .diagnostics
        .iter()
        .filter(|d| {
            d.message.contains("PG-POLICY-WITHOUT-RLS")
                || d.message.contains("PG-RLS-WITHOUT-POLICY")
        })
        .collect();
    assert!(
        rls_diags.is_empty(),
        "expected 0 RLS violations: {rls_diags:?}"
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn plsql_package_spec_body_mismatch_and_utplsql_empty_test_gf076()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("plsql-pkg")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::create_dir_all(dir.join("tests"))?;
    fs::write(
        dir.join("src/billing.pks"),
        "CREATE OR REPLACE PACKAGE billing AUTHID CURRENT_USER AS\n  PROCEDURE close_month(p_month IN DATE);\nEND billing;\n/\n",
    )?;
    fs::write(
        dir.join("src/billing.pkb"),
        "CREATE OR REPLACE PACKAGE BODY billing AS\n  PROCEDURE other_proc IS\n  BEGIN\n    NULL;\n  END other_proc;\nEND billing;\n/\n",
    )?;
    fs::write(
        dir.join("tests/test_billing.pkb"),
        "CREATE OR REPLACE PACKAGE BODY test_billing AS\n  --%test(empty test)\n  PROCEDURE test_empty IS\n  BEGIN\n    NULL;\n  END test_empty;\nEND test_billing;\n/\n",
    )?;
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.message.contains("PLSQL-PACKAGE-SPEC-MISMATCH")),
        "expected PLSQL-PACKAGE-SPEC-MISMATCH: {:?}",
        rep.diagnostics
    );
    assert!(
        rep.diagnostics.iter().any(|d| d.rule_id == "GF076"),
        "expected GF076 on assertion-free utPLSQL test: {:?}",
        rep.diagnostics
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_init_for_plsql_postgresql_and_mariadb_pass_with_zero_violations()
-> Result<(), Box<dyn std::error::Error>> {
    let cases: [(&str, Vec<LanguageId>, Vec<FrameworkId>); 3] = [
        ("plsql", vec![LanguageId::PlSql], vec![]),
        ("pg", vec![LanguageId::Sql], vec![FrameworkId::PostgreSql]),
        ("mariadb", vec![LanguageId::Sql], vec![FrameworkId::MariaDb]),
    ];
    for (tag, langs, fws) in cases {
        let dir = temp_ws(tag)?;
        let req = InitRequest {
            root: dir.clone(),
            project_name: Some(format!("demo-{tag}")),
            archetype: None,
            languages: langs.into_iter().collect::<BTreeSet<_>>(),
            features: BTreeSet::new(),
            frameworks: fws.into_iter().collect::<BTreeSet<_>>(),
            run_git_init: false,
        };
        run_init_with_sync_mode(&req, SkillsFetchMode::Offline)?;
        let mut cfg = Config::strict_defaults();
        cfg.integrity.require_lock = false;
        let rep = run_check(&dir, ScanScope::All, &cfg)?;
        let non_tool: Vec<_> = rep
            .diagnostics
            .iter()
            .filter(|d| d.rule_id != "GF043")
            .map(|d| format!("{} {} {}", d.rule_id, d.path, d.message))
            .collect();
        assert_eq!(
            non_tool,
            Vec::<String>::new(),
            "golden init failed for {tag}"
        );
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
