//! Unit and mutation-killer tests for `crates/gf-rules/src/web_edge_workspace_checks.rs`.

use std::collections::BTreeSet;

use gf_core::{
    A11yViolation, CloudflareViolation, Config, Diagnostic, DrizzleViolation, LangRuleFinding,
    Language, ProjectContext, RepoPath, Span, SqlDialectKind, UastDocument, WebEdgeSummary,
    WranglerSummary,
};
use gf_rules::evaluate_workspace_sorted;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn make_doc(
    path: &str,
    lang: Language,
    we: WebEdgeSummary,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, lang, 20, 15)?;
    doc.web_edge = Some(we);
    Ok(doc)
}

fn eval_ws(docs: &[UastDocument]) -> Vec<Diagnostic> {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();
    evaluate_workspace_sorted(docs, &ctx, &cfg)
}

fn sample_wrangler_summary(sp2: Span, sp5: Span, nodejs_compat: bool) -> WranglerSummary {
    let mut wr = WranglerSummary {
        has_nodejs_compat: nodejs_compat,
        has_d1: true,
        declared_bindings: BTreeSet::new(),
        d1_r2_bindings: Vec::new(),
        span: Span::FIRST,
    };
    wr.declared_bindings.insert(String::from("DB"));
    wr.declared_bindings.insert(String::from("UNUSED_R2"));
    wr.d1_r2_bindings.push((String::from("DB"), sp2));
    wr.d1_r2_bindings.push((String::from("UNUSED_R2"), sp5));
    wr
}

#[rustfmt::skip]
#[test]
fn wrangler_bindings_unused_d1_r2_and_nodejs_compat_guards_are_exact() -> TestResult {
    let (sp2, sp5, sp9) = (Span::new(2, 2)?, Span::new(5, 5)?, Span::new(9, 9)?);
    let wrangler_doc = make_doc("wrangler.toml", Language::Toml, WebEdgeSummary {
        wrangler: Some(sample_wrangler_summary(sp2, sp5, false)),
        ..WebEdgeSummary::default()
    })?;
    let idle_worker = make_doc("src/index.ts", Language::TypeScript, WebEdgeSummary::default())?;
    assert_eq!(eval_ws(&[wrangler_doc.clone(), idle_worker]), Vec::new());

    let mut env_types_we = WebEdgeSummary::default();
    env_types_we.documented_env_keys.insert(String::from("DOC_KV"));
    let env_types_doc = make_doc("src/env.d.ts", Language::TypeScript, env_types_we)?;

    let mut worker_we = WebEdgeSummary::default();
    for (b, sp) in [("DB", sp2), ("DOC_KV", sp2), ("GHOST_KV", sp5), ("ALREADY_FLAGGED", sp9)] {
        worker_we.env_bindings_accessed.push((String::from(b), sp));
    }
    worker_we.node_colon_imports.push((String::from("node:crypto"), sp2));
    worker_we.node_colon_imports.push((String::from("node:buffer"), sp9));

    let mut worker_doc = make_doc("src/worker.ts", Language::TypeScript, worker_we)?;
    worker_doc.lang_rule_findings.push(LangRuleFinding::new(CloudflareViolation::BindingCoherence, sp9, String::from("dup")));
    worker_doc.lang_rule_findings.push(LangRuleFinding::new(CloudflareViolation::ForbiddenNodeApi, sp9, String::from("dup")));

    let diags = eval_ws(&[wrangler_doc, env_types_doc, worker_doc.clone()]);
    assert!(diags.iter().any(|d| d.rule_id == "GF080" && d.message.contains("env.GHOST_KV") && d.span == sp5));
    assert!(!diags.iter().any(|d| d.message.contains("env.ALREADY_FLAGGED") || d.message.contains("env.DOC_KV")));
    assert!(diags.iter().any(|d| d.rule_id == "GF080" && d.message.contains("UNUSED_R2") && d.span == sp5));
    assert!(!diags.iter().any(|d| d.message.contains("binding `DB` declared")));
    assert!(diags.iter().any(|d| d.rule_id == "GF080" && d.message.contains("node:crypto") && d.span == sp2));
    assert!(!diags.iter().any(|d| d.message.contains("node:buffer")));
    assert!(diags.iter().any(|d| d.rule_id == "GF081" && d.path.as_str() == "wrangler.toml"));

    let compat_wrangler = make_doc("wrangler.toml", Language::Toml, WebEdgeSummary {
        wrangler: Some(sample_wrangler_summary(sp2, sp5, true)),
        ..WebEdgeSummary::default()
    })?;
    assert!(!eval_ws(&[compat_wrangler, worker_doc]).iter().any(|d| d.message.contains("nodejs_compat")));
    Ok(())
}

#[rustfmt::skip]
#[test]
fn drizzle_dialect_resolution_compatibility_and_deduplication_are_exact() -> TestResult {
    let (sp3, sp7) = (Span::new(3, 3)?, Span::new(7, 7)?);
    let mut schema_we = WebEdgeSummary::default();
    for (d, sp) in [(SqlDialectKind::Postgres, sp3), (SqlDialectKind::Sqlite, sp3), (SqlDialectKind::MySql, sp7)] {
        schema_we.drizzle_dialect_imports.push((d, sp));
    }
    let mut schema_doc = make_doc("src/db/schema.ts", Language::TypeScript, schema_we)?;
    schema_doc.lang_rule_findings.push(LangRuleFinding::new(DrizzleViolation::DialectMismatch, sp7, String::from("dup")));

    assert_eq!(eval_ws(&[schema_doc.clone()]), Vec::new());

    let pg_cfg = make_doc("drizzle.config.ts", Language::TypeScript, WebEdgeSummary {
        configured_drizzle_dialect: Some(SqlDialectKind::Postgres),
        ..WebEdgeSummary::default()
    })?;
    let pg_diags = eval_ws(&[pg_cfg, schema_doc.clone()]);
    assert_eq!(pg_diags.len(), 1);
    assert!(pg_diags[0].message.contains("imports `sqlite` Drizzle core in a `postgres` workspace") && pg_diags[0].span == sp3);

    let d1_wrangler = make_doc("wrangler.toml", Language::Toml, WebEdgeSummary {
        wrangler: Some(sample_wrangler_summary(sp3, sp7, false)),
        ..WebEdgeSummary::default()
    })?;
    let d1_diags = eval_ws(&[d1_wrangler, schema_doc]);
    assert!(d1_diags.iter().any(|d| d.message.contains("imports `postgres` Drizzle core in a `d1` workspace")));
    Ok(())
}

#[rustfmt::skip]
#[test]
fn forme_reduced_motion_domain_filtering_and_deduplication_are_exact() -> TestResult {
    let (sp4, sp8) = (Span::new(4, 4)?, Span::new(8, 8)?);
    let mut css_we = WebEdgeSummary::default();
    css_we.forme_spatial_motion_spans.extend([sp4, sp8]);
    let mut forme_doc = make_doc("styles/motion.css", Language::Css, css_we.clone())?;
    forme_doc.lang_rule_findings.push(LangRuleFinding::new(A11yViolation::MediaFormMotion, sp8, String::from("dup")));

    let code_doc = make_doc("src/app.ts", Language::TypeScript, css_we)?;
    let diags = eval_ws(&[forme_doc.clone(), code_doc]);
    assert_eq!(diags.len(), 1);
    assert_eq!((diags[0].path.as_str(), diags[0].span), ("styles/motion.css", sp4));

    let reset_doc = make_doc("styles/reset.css", Language::Css, WebEdgeSummary {
        forme_has_reduced_motion: true,
        ..WebEdgeSummary::default()
    })?;
    assert_eq!(eval_ws(&[forme_doc, reset_doc]), Vec::new());
    Ok(())
}
