//! Direct unit tests in `gf-rules` for `kiss_yagni_checks.rs`, `boundary_checks.rs`,
//! `import_resolver.rs`, and `rust_import_resolver.rs` (`GF070`–`GF075`).

use std::slice;

use gf_core::{
    BoundaryRule, BypassFinding, Config, ContractDecl, ContractImpl, ImportDecl, KissYagniFinding,
    KissYagniKind, Language, ProjectContext, RepoPath, Span, UastDocument, YagniRule,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn doc_at(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 8)?)
}

fn ky(kind: KissYagniKind, measured: u32) -> KissYagniFinding {
    KissYagniFinding {
        kind,
        measured,
        detail: "item".into(),
        span: Span::FIRST,
    }
}

fn imp(src: &str) -> ImportDecl {
    ImportDecl {
        module_source: src.into(),
        symbols: Vec::new(),
        span: Span::FIRST,
    }
}

#[test]
fn kiss_yagni_and_bypass_enforce_exact_thresholds() -> Res {
    let mut cfg = Config::strict_defaults();
    let mut doc = doc_at("src/logic.ts", Language::TypeScript)?;
    doc.total_lines = cfg.limits.max_file_lines;
    doc.bypass_findings.push(BypassFinding {
        directive: "eslint-disable".into(),
        span: Span::FIRST,
    });
    doc.kiss_yagni_findings = vec![
        ky(KissYagniKind::NestedTernary, 0),
        ky(KissYagniKind::NestedTernary, 2),
        ky(KissYagniKind::PropertyChain, 4),
        ky(KissYagniKind::PropertyChain, 6),
        ky(KissYagniKind::WildcardReExport, 1),
        ky(KissYagniKind::ExcessiveBoolParams, 1),
        ky(KissYagniKind::ExcessiveBoolParams, 3),
        ky(KissYagniKind::TodoMarker, 1),
    ];
    let diags = evaluate_document(&doc, &cfg);
    assert!(!diags.iter().any(|d| d.rule_id == "GF001"));
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF070").count(), 1);
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF073").count(), 2);
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF074").count(), 3);

    cfg.clean_code.forbid_todos = false;
    cfg.clean_code
        .yagni_rules
        .remove(&YagniRule::WildcardExports);
    assert_eq!(
        evaluate_document(&doc, &cfg)
            .iter()
            .filter(|d| d.rule_id == "GF074")
            .count(),
        1
    );
    let mut test_todo = doc_at("tests/helpers/t.ts", Language::TypeScript)?;
    test_todo
        .kiss_yagni_findings
        .push(ky(KissYagniKind::TodoMarker, 1));
    assert_eq!(
        evaluate_document(&test_todo, &Config::strict_defaults()),
        Vec::new()
    );
    Ok(())
}

#[test]
fn single_impl_contracts_enforce_min_two_implementors() -> Res {
    let mut svc = doc_at("src/repo.rs", Language::Rust)?;
    svc.contracts.push(ContractDecl {
        name: "Repo".into(),
        method_count: 1,
        is_public: true,
        span: Span::FIRST,
    });
    svc.contract_impls.insert(ContractImpl {
        contract: "Repo".into(),
        implementor: "SqlRepo".into(),
    });
    let ctx = ProjectContext::default();
    let def = Config::strict_defaults();
    assert!(
        evaluate_workspace_sorted(slice::from_ref(&svc), &ctx, &def)
            .iter()
            .any(|d| d.rule_id == "GF075")
    );

    svc.contract_impls.insert(ContractImpl {
        contract: "Repo".into(),
        implementor: "MemRepo".into(),
    });
    assert!(
        !evaluate_workspace_sorted(slice::from_ref(&svc), &ctx, &def)
            .iter()
            .any(|d| d.rule_id == "GF075")
    );

    let mut lib_entry = doc_at("src/lib.rs", Language::Rust)?;
    lib_entry.contracts = svc.contracts.clone();
    assert!(
        !evaluate_workspace_sorted(&[lib_entry], &ctx, &def)
            .iter()
            .any(|d| d.rule_id == "GF075")
    );

    let mut disabled = Config::strict_defaults();
    disabled
        .clean_code
        .yagni_rules
        .remove(&YagniRule::SingleImplTraits);
    svc.contract_impls.clear();
    assert!(
        !evaluate_workspace_sorted(&[svc], &ctx, &disabled)
            .iter()
            .any(|d| d.rule_id == "GF075")
    );
    Ok(())
}

#[test]
fn boundaries_and_import_resolvers_enforce_gf071_and_gf072() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.boundaries = vec![BoundaryRule {
        from: "crates/core/**".into(),
        allow: vec!["crates/core/**".into(), "alloc".into()],
        deny: vec![
            "gf_engine".into(),
            "src/forbidden".into(),
            "crates/core/src/sub/secret".into(),
        ],
        reason: "pure core".into(),
    }];
    let mut core_doc = doc_at("crates/core/src/sub/m.rs", Language::Rust)?;
    core_doc.imports = vec![
        imp("gf_engine::cache"),
        imp("src/forbidden/mod"),
        imp("./secret"),
        imp("std::fs"),
        imp("alloc::vec"),
    ];
    let b_diags = evaluate_workspace_sorted(&[core_doc], &ProjectContext::default(), &cfg);
    assert_eq!(b_diags.iter().filter(|d| d.rule_id == "GF072").count(), 4);
    assert!(!b_diags.iter().any(|d| d.message.contains("alloc::vec")));

    let mut ts_a = doc_at("src/a.ts", Language::TypeScript)?;
    ts_a.imports.push(imp("@/b"));
    let mut ts_b = doc_at("src/b.ts", Language::TypeScript)?;
    ts_b.imports.push(imp("./a"));
    let mut py_a = doc_at("pkg/sub/a.py", Language::Python)?;
    py_a.imports.push(imp("..b"));
    let mut py_b = doc_at("pkg/b.py", Language::Python)?;
    py_b.imports.push(imp(".sub.a"));
    let mut rs_mod = doc_at("crates/x/src/net/mod.rs", Language::Rust)?;
    rs_mod.imports.push(imp("crate::{self, peer}"));
    let mut rs_peer = doc_at("crates/x/src/peer.rs", Language::Rust)?;
    rs_peer.imports.push(imp("crate::net"));
    let cyc = evaluate_workspace_sorted(
        &[ts_a, ts_b, py_a, py_b, rs_mod, rs_peer],
        &ProjectContext::default(),
        &Config::strict_defaults(),
    );
    assert_eq!(cyc.iter().filter(|d| d.rule_id == "GF071").count(), 6);
    Ok(())
}
