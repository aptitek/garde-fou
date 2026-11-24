//! Direct unit tests in `gf-rules` for `asset_and_script_checks.rs` and `i18n_checks.rs` (`GF062`, `GF064`, `GF065`).

use std::slice;

use gf_core::{
    AssetDocument, Config, FeatureId, ImportDecl, Language, ProjectContext, RepoPath, Span,
    TexteDocument, UastDocument, UiCheckFinding, UiCheckKind, Validation,
};
use gf_rules::{evaluate_workspace, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn doc_at(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 8)?)
}

fn imp(src: &str) -> ImportDecl {
    ImportDecl {
        module_source: src.into(),
        symbols: Vec::new(),
        span: Span::FIRST,
    }
}

#[test]
fn workspace_assets_resolve_relative_and_public_links_and_detect_orphans() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.entrypoints = vec![String::from("assets/entry_banner.png")];
    let ctx = ProjectContext::default();

    let mk_asset = |p: &str| -> Result<UastDocument, Box<dyn std::error::Error>> {
        let mut d = doc_at(p, Language::BinaryAsset)?;
        d.asset = Some(AssetDocument {
            byte_size: 128,
            extension: "png".into(),
            hazards: Vec::new(),
        });
        Ok(d)
    };
    let mut page = doc_at("docs/sub/page.md", Language::Markdown)?;
    page.asset_refs = [
        "../img/rel.png",
        "/assets/abs.png",
        "hero.png",
        "./missing.png",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let docs = vec![
        page,
        mk_asset("docs/img/rel.png")?,
        mk_asset("assets/abs.png")?,
        mk_asset("public/hero.png")?,
        mk_asset("public/favicon.ico")?,
        mk_asset("assets/entry_banner.png")?,
        mk_asset("assets/orphan.png")?,
    ];
    let diags = evaluate_workspace_sorted(&docs, &ctx, &cfg);
    let gf064: Vec<_> = diags.iter().filter(|d| d.rule_id == "GF064").collect();
    assert_eq!(gf064.len(), 2);
    assert!(gf064.iter().any(|d| d.message.contains("missing.png")));
    assert!(gf064.iter().any(|d| d.path.as_str() == "assets/orphan.png"));
    Ok(())
}

#[test]
fn workspace_scripts_forbid_code_imports_and_verify_test_coverage() -> Res {
    let mut code = doc_at("src/app.ts", Language::TypeScript)?;
    code.imports = vec![imp("../scripts/deploy"), imp("./util")];
    code.script_refs.insert("scripts/uncovered.sh".into());

    let mut test_doc = doc_at("tests/scripts_smoke.ts", Language::TypeScript)?;
    test_doc.script_refs = [
        "scripts/by_full.sh",
        "by_name.sh",
        "scripts/by_stem",
        "tools/by_tool",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    test_doc.imports.push(imp("../scripts/by_import.ts"));

    let docs = vec![
        code,
        test_doc,
        doc_at("scripts/deploy.ts", Language::TypeScript)?,
        doc_at("scripts/uncovered.sh", Language::Shell)?,
        doc_at("scripts/by_full.sh", Language::Shell)?,
        doc_at("scripts/by_name.sh", Language::Shell)?,
        doc_at("scripts/by_stem.py", Language::Python)?,
        doc_at("tools/by_tool.sh", Language::Shell)?,
        doc_at("scripts/by_import.ts", Language::TypeScript)?,
    ];
    let gf065: Vec<_> =
        evaluate_workspace_sorted(&docs, &ProjectContext::default(), &Config::default())
            .into_iter()
            .filter(|d| d.rule_id == "GF065")
            .collect();
    assert_eq!(gf065.len(), 3);
    assert_eq!(
        gf065
            .iter()
            .filter(|d| d.path.as_str() == "src/app.ts")
            .count(),
        1
    );
    assert!(
        gf065
            .iter()
            .any(|d| d.path.as_str() == "scripts/uncovered.sh")
    );
    assert!(gf065.iter().any(|d| d.path.as_str() == "scripts/deploy.ts"));
    Ok(())
}

fn mk_loc(
    path: &str,
    loc: &str,
    keys: &[&str],
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut d = doc_at(path, Language::Json)?;
    d.texte = Some(TexteDocument {
        locale: loc.into(),
        keys: keys.iter().copied().map(String::from).collect(),
        forbidden_values: Vec::new(),
    });
    Ok(d)
}

#[test]
fn workspace_i18n_enforces_symmetry_missing_dead_and_jsx_strings() -> Res {
    let cfg = Config::strict_defaults();
    let mut ctx = ProjectContext::default();
    let plain_code = doc_at("src/main.rs", Language::Rust)?;
    assert_eq!(
        evaluate_workspace(&[plain_code], &ctx, &cfg),
        Validation::Valid(())
    );
    let single_en = mk_loc("locales/en.json", "en", &["nav.home"])?;
    assert_eq!(
        evaluate_workspace_sorted(slice::from_ref(&single_en), &ctx, &cfg),
        Vec::new()
    );
    let sym_fr = mk_loc("locales/fr.json", "fr", &["nav.home"])?;
    let mut clean_ui = doc_at("src/Clean.tsx", Language::Tsx)?;
    clean_ui
        .translation_keys_used
        .insert(String::from("nav.home"));
    assert_eq!(
        evaluate_workspace_sorted(&[single_en.clone(), sym_fr, clean_ui], &ctx, &cfg),
        Vec::new()
    );

    let asym_fr = mk_loc("locales/fr.json", "fr", &["nav.home", "nav.extra"])?;
    let asym_es = mk_loc("locales/es.json", "es", &["nav.home"])?;
    assert_ne!(
        evaluate_workspace_sorted(&[single_en.clone(), asym_fr.clone(), asym_es], &ctx, &cfg).len(),
        0
    );
    let mut ui = doc_at("src/Header.tsx", Language::Tsx)?;
    ui.translation_keys_used = ["nav.home", "nav.missing"]
        .into_iter()
        .map(String::from)
        .collect();
    ui.ui_checks.push(UiCheckFinding {
        kind: UiCheckKind::UntranslatedText,
        detail: "Raw".into(),
        span: Span::FIRST,
    });
    ctx.features.insert(FeatureId::I18n);
    assert_ne!(
        evaluate_workspace_sorted(slice::from_ref(&ui), &ctx, &cfg).len(),
        0
    );
    let diags = evaluate_workspace_sorted(&[single_en, asym_fr, ui], &ctx, &cfg);
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF062").count(), 4);
    Ok(())
}
