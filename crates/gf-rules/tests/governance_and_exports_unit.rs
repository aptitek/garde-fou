//! Direct unit tests in `gf-rules` for `workspace_checks.rs` (`GF021`)
//! and `governance_checks.rs` (`GF070`, `GF078`).

use gf_core::{
    AgenticSyncStatus, Config, EffectSighting, EffectSightingKind, ExportDecl, FrameworkId,
    Language, LockStatus, ProjectContext, RepoPath, Span, UastDocument, Validation,
};
use gf_rules::{
    check_agentic_governance, check_architecture_purity, check_lock_integrity,
    evaluate_workspace_sorted,
};

type Res = Result<(), Box<dyn std::error::Error>>;

fn doc_at(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 8)?)
}

fn exp(sym: &str, internal: bool) -> ExportDecl {
    ExportDecl {
        symbol: sym.into(),
        is_crate_internal: internal,
        span: Span::FIRST,
    }
}

#[test]
fn dead_exports_enforce_crate_prefixes_self_refs_and_entrypoints() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.entrypoints = vec![String::from("src/custom_entry.rs")];
    let mut ctx = ProjectContext::default();
    ctx.frameworks.insert(FrameworkId::Astro);

    let mut ca = doc_at("crates/ab/src/internal.rs", Language::Rust)?;
    ca.exports.push(exp("secret_fn", true));
    let mut cb = doc_at("crates/abc/src/other.rs", Language::Rust)?;
    cb.referenced_symbols.insert("secret_fn".into());
    assert_eq!(
        evaluate_workspace_sorted(&[ca.clone(), cb], &ctx, &cfg)
            .iter()
            .filter(|d| d.rule_id == "GF021")
            .count(),
        1
    );

    let mut self_only = doc_at("src/self_ref.rs", Language::Rust)?;
    self_only.exports.push(exp("recurse", false));
    self_only.referenced_symbols.insert("recurse".into());
    let mut custom_ep = doc_at("src/custom_entry.rs", Language::Rust)?;
    custom_ep.exports.push(exp("ep_api", false));
    let mut astro_page = doc_at("src/pages/home.tsx", Language::Tsx)?;
    astro_page.exports.push(exp("Page", false));
    let mut script_doc = doc_at("scripts/run.ts", Language::TypeScript)?;
    script_doc.exports.push(exp("helper", false));
    let mut df = doc_at("src/Dockerfile", Language::Dockerfile)?;
    df.exports.push(exp("ignored", false));

    let diags = evaluate_workspace_sorted(
        &[self_only, custom_ep, astro_page, script_doc, df],
        &ctx,
        &cfg,
    );
    let gf021: Vec<&str> = diags
        .iter()
        .filter(|d| d.rule_id == "GF021")
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(gf021, vec!["src/self_ref.rs"]);
    Ok(())
}

#[test]
fn governance_lock_and_agentic_sync_enforce_gf070() {
    assert_eq!(
        check_lock_integrity(&LockStatus::Verified),
        Validation::Valid(())
    );
    assert!(!check_lock_integrity(&LockStatus::Missing).is_valid());
    assert!(
        !check_lock_integrity(&LockStatus::Tampered {
            expected: "a".into(),
            actual: "b".into()
        })
        .is_valid()
    );
    assert_eq!(
        check_agentic_governance(&AgenticSyncStatus::Synced),
        Validation::Valid(())
    );
    for bad in [
        AgenticSyncStatus::MissingAgentsMd,
        AgenticSyncStatus::DriftedAgentsMd,
        AgenticSyncStatus::MissingOrOutdatedCoreSkill,
    ] {
        assert!(!check_agentic_governance(&bad).is_valid());
    }
}

#[test]
fn architecture_purity_enforces_independent_no_std_io_and_console_axes() -> Res {
    let eff = |kind: EffectSightingKind| EffectSighting {
        kind,
        detail: "e".into(),
        span: Span::FIRST,
    };
    let root_lib = doc_at("src/lib.rs", Language::Rust)?;
    let root_mod = doc_at("src/other.rs", Language::Rust)?;
    let mut ns_only = Config::strict_defaults();
    ns_only.architecture.no_std_crates.insert("root".into());
    match check_architecture_purity(&[root_lib, root_mod], &ns_only) {
        Validation::Valid(()) => assert_eq!(0, 1),
        Validation::Invalid(d) => assert_eq!(d.len(), 1),
    }

    let mut io_only = Config::strict_defaults();
    io_only
        .architecture
        .io_allowed_crates
        .insert("io_ok".into());
    let mut ok_io = doc_at("crates/io_ok/src/lib.rs", Language::Rust)?;
    ok_io
        .effect_sightings
        .push(eff(EffectSightingKind::IoImportOrCall));
    let mut bad_io = doc_at("crates/pure_x/src/lib.rs", Language::Rust)?;
    bad_io
        .effect_sightings
        .push(eff(EffectSightingKind::IoImportOrCall));
    match check_architecture_purity(&[ok_io, bad_io], &io_only) {
        Validation::Valid(()) => assert_eq!(0, 1),
        Validation::Invalid(d) => assert_eq!(d.len(), 1),
    }

    let mut con_only = Config::strict_defaults();
    con_only
        .architecture
        .console_allowed_crates
        .insert("cli_ok".into());
    let mut bad_con = doc_at("crates/pure_x/src/lib.rs", Language::Rust)?;
    bad_con
        .effect_sightings
        .push(eff(EffectSightingKind::ConsoleOutput));
    let mut out_exempt = doc_at("crates/gf-cli/src/output.rs", Language::Rust)?;
    out_exempt
        .effect_sightings
        .push(eff(EffectSightingKind::ConsoleOutput));
    match check_architecture_purity(&[bad_con, out_exempt], &con_only) {
        Validation::Valid(()) => assert_eq!(0, 1),
        Validation::Invalid(d) => assert_eq!(d.len(), 1),
    }
    Ok(())
}
