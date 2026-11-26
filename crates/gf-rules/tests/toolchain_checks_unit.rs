//! Unit tests for `gf-rules` workspace toolchain (`GRADLE-USE-VERSION-CATALOG`) and MERN
//! frontend/backend boundary (`GF072`) checks.

use std::collections::BTreeSet;

use gf_core::{
    Config, FrameworkId, ImportDecl, Language, ProjectContext, RepoPath, Span, ToolchainSummary,
    UastDocument,
};
use gf_rules::evaluate_workspace;

fn eval(docs: &[UastDocument], fws: &[FrameworkId]) -> Vec<gf_core::Diagnostic> {
    let ctx = ProjectContext {
        frameworks: fws.iter().copied().collect::<BTreeSet<_>>(),
        ..ProjectContext::default()
    };
    evaluate_workspace(docs, &ctx, &Config::strict_defaults())
        .into_result()
        .err()
        .unwrap_or_default()
}

#[test]
fn gradle_version_catalog_triggers_only_when_catalog_present()
-> Result<(), Box<dyn std::error::Error>> {
    let mut build = UastDocument::new(RepoPath::new("build.gradle.kts")?, Language::Kotlin, 5, 5)?;
    build.toolchain_meta = Some(ToolchainSummary {
        hardcoded_gradle_deps: vec![(Span::FIRST, String::from("io.ktor:ktor-core:3.0.0"))],
        ..ToolchainSummary::default()
    });
    assert_eq!(
        eval(std::slice::from_ref(&build), &[])
            .iter()
            .filter(|d| d.rule_id == "GF081")
            .count(),
        0
    );

    let mut catalog = UastDocument::new(
        RepoPath::new("gradle/libs.versions.toml")?,
        Language::Toml,
        3,
        3,
    )?;
    catalog.toolchain_meta = Some(ToolchainSummary {
        is_gradle_version_catalog: true,
        ..ToolchainSummary::default()
    });
    let diags = eval(&[build, catalog], &[]);
    assert!(
        diags
            .iter()
            .any(|d| d.rule_id == "GF081" && d.message.contains("GRADLE-USE-VERSION-CATALOG"))
    );
    Ok(())
}

fn mk_imp_doc(
    path: &str,
    lang: Language,
    module: &str,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, lang, 3, 3)?;
    doc.imports.push(ImportDecl {
        module_source: String::from(module),
        symbols: Vec::new(),
        span: Span::FIRST,
        is_type_only: false,
    });
    Ok(doc)
}

#[test]
fn mern_boundary_blocks_backend_imports_only_in_frontend_modules()
-> Result<(), Box<dyn std::error::Error>> {
    let web = mk_imp_doc("apps/web/src/App.tsx", Language::Tsx, "mongoose")?;
    let client = mk_imp_doc("client/page.ts", Language::TypeScript, "node:fs/promises")?;
    let api = mk_imp_doc("apps/api/src/db.ts", Language::TypeScript, "express")?;
    let clean_web = mk_imp_doc("apps/web/src/Clean.tsx", Language::Tsx, "react")?;
    let widget = mk_imp_doc("src/Widget.jsx", Language::Jsx, "express")?;

    let diags = eval(
        &[web.clone(), client, api, clean_web, widget],
        &[FrameworkId::React, FrameworkId::Express],
    );
    let gf072: Vec<&str> = diags
        .iter()
        .filter(|d| d.rule_id == "GF072")
        .map(|d| d.path.as_str())
        .collect();
    assert!(gf072.contains(&"apps/web/src/App.tsx"));
    assert!(gf072.contains(&"client/page.ts"));
    assert!(gf072.contains(&"src/Widget.jsx"));
    assert!(!gf072.contains(&"apps/api/src/db.ts"));
    assert!(!gf072.contains(&"apps/web/src/Clean.tsx"));
    assert_eq!(
        eval(&[web], &[FrameworkId::React])
            .iter()
            .filter(|d| d.rule_id == "GF072")
            .count(),
        0
    );
    Ok(())
}
