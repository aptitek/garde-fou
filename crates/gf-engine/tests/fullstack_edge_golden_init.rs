//! Phase 4.2: E2E Golden Invariant (`garde init` -> `garde check --no-cache == 0 violations`),
//! cross-file `CF-BINDING-COHERENCE` / `CF-WRANGLER-CONFIG` / `DRIZZLE-DIALECT-MISMATCH`,
//! and Tier 3 output parsers (`parse_astro_check_output`, `parse_html_validate_json`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, ExternalToolKind, FeatureId, FrameworkId, LanguageId};
use gf_engine::{
    CheckOptions, InitRequest, ScanScope, parse_astro_check_output, parse_html_validate_json,
    run_check, run_check_with_options, run_init,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

static EDGE_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_edge_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = EDGE_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-edge-golden-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_file(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(p) = full.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(full, content)
}

#[rustfmt::skip]
#[test]
fn golden_invariant_fullstack_edge_react_router_workers_drizzle_panda_m3e_w3c() -> TestResult {
    let dir = fresh_edge_dir("rr-cf-drizzle-panda-m3e")?;
    let req = InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("edge-commerce")),
        languages: BTreeSet::from([LanguageId::TypeScript, LanguageId::Sql]),
        features: BTreeSet::from([FeatureId::Ui, FeatureId::PandaCss, FeatureId::Material3Expressive, FeatureId::W3c]),
        frameworks: BTreeSet::from([FrameworkId::ReactRouter, FrameworkId::CloudflareWorkers, FrameworkId::Drizzle]),
        run_git_init: true,
    };
    let init_rep = run_init(&req)?;
    assert!(init_rep.git_initialized);

    for rel in [
        "package.json", "tsconfig.json", "panda.config.ts", "wrangler.toml", "drizzle.config.ts",
        "migrations/0001_init.sql", "styles/tokens.css", "styles/app.css", "AGENTS.md", "garde.lock",
    ] {
        assert!(dir.join(rel).is_file(), "missing expected starter file `{rel}`");
    }
    let gitignore = fs::read_to_string(dir.join(".gitignore"))?;
    for ig in ["styled-system/", ".wrangler/", ".react-router/"] {
        assert!(gitignore.contains(ig), "missing `.gitignore` entry `{ig}`");
    }
    let direct = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    assert!(direct.diagnostics.is_empty(), "direct check failed: {:#?}", direct.diagnostics);
    let full = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(full.diagnostics.is_empty(), "configured check failed: {:#?}", full.diagnostics);
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_invariant_astro_ui_w3c_content_stack() -> TestResult {
    let dir = fresh_edge_dir("astro-ui-w3c-content")?;
    let req = InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("astro-docs")),
        languages: BTreeSet::from([LanguageId::Astro, LanguageId::TypeScript]),
        features: BTreeSet::from([FeatureId::Ui, FeatureId::W3c, FeatureId::Content]),
        frameworks: BTreeSet::from([FrameworkId::Astro]),
        run_git_init: true,
    };
    let init_rep = run_init(&req)?;
    assert!(init_rep.git_initialized);
    for rel in [
        "package.json",
        "src/pages/index.astro",
        "content/index.md",
        "styles/tokens.css",
        "assets/logo.svg",
        "AGENTS.md",
    ] {
        assert!(dir.join(rel).is_file(), "missing `{rel}`");
    }

    let full = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        full.diagnostics.is_empty(),
        "astro stack check failed: {:#?}",
        full.diagnostics
    );

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[rustfmt::skip]
#[test]
fn cross_file_wrangler_bindings_nodejs_compat_and_drizzle_dialect_coherence() -> TestResult {
    let dir = fresh_edge_dir("cf-drizzle-cross-file")?;
    write_file(
        &dir,
        "wrangler.toml",
        "name = \"edge-api\"\ncompatibility_date = \"invalid-date\"\n[vars]\nSTRIPE_SECRET_KEY = \"sk_test_inline\"\n[[d1_databases]]\nbinding = \"DB\"\ndatabase_name = \"prod\"\nremote = true\n[[r2_buckets]]\nbinding = \"UNUSED_BUCKET\"\nbucket_name = \"media\"\n",
    )?;
    write_file(
        &dir,
        "src/worker.ts",
        "import crypto from \"node:crypto\";\nimport { pgTable, integer } from \"drizzle-orm/pg-core\";\nexport const t = pgTable(\"t\", { id: integer(\"id\").primaryKey() });\nexport function handle(env: { DB: unknown; MISSING_KV: unknown }) {\n  return [crypto, t, env.DB, env.MISSING_KV];\n}\n",
    )?;
    write_file(
        &dir,
        "tests/worker.test.ts",
        "import { expect, test } from \"vitest\";\nimport { handle } from \"../src/worker\";\ntest(\"handles worker env\", () => {\n  expect(handle({ DB: 1, MISSING_KV: 2 }).length).toBe(4);\n});\n",
    )?;

    let rep = run_check(&dir, ScanScope::All, &Config::strict_defaults())?;
    let msgs: Vec<&str> = rep.diagnostics.iter().map(|d| d.message.as_str()).collect();
    for (rule_tag, needle) in [
        ("[CF-WRANGLER-CONFIG]", "compatibility_date"),
        ("[CF-WRANGLER-CONFIG]", "STRIPE_SECRET_KEY"),
        ("[CF-WRANGLER-CONFIG]", "remote = true"),
        ("[CF-WRANGLER-CONFIG]", "nodejs_compat"),
        ("[CF-FORBIDDEN-NODE-API]", "node:crypto"),
        ("[CF-BINDING-COHERENCE]", "env.MISSING_KV"),
        ("[CF-BINDING-COHERENCE]", "UNUSED_BUCKET"),
        ("[DRIZZLE-DIALECT-MISMATCH]", "pg-core"),
    ] {
        assert!(msgs.iter().any(|m| m.contains(rule_tag) && m.contains(needle)), "missing {rule_tag} + {needle} in {msgs:#?}");
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn tier3_astro_check_and_html_validate_parsers_normalize_findings() -> TestResult {
    let root = Path::new("/repo");
    let astro_raw = "\
\u{1b}[31msrc/pages/index.astro:14:7 - error ts(2322): Type 'number' is not assignable to type 'string'.\u{1b}[0m
src/layouts/Base.astro(8,3): error astro(4000): Unknown client directive.
";
    let astro_findings = parse_astro_check_output(astro_raw, root)?;
    assert_eq!(astro_findings.len(), 2);
    assert_eq!(astro_findings[0].tool, ExternalToolKind::AstroCheck);
    assert_eq!(astro_findings[0].path.as_str(), "src/pages/index.astro");
    assert_eq!((astro_findings[0].line, astro_findings[0].column), (14, 7));
    assert_eq!(astro_findings[0].native_code, "ts(2322)");
    assert_eq!(astro_findings[1].path.as_str(), "src/layouts/Base.astro");
    assert_eq!((astro_findings[1].line, astro_findings[1].column), (8, 3));

    let html_json = r#"[
      {
        "filePath": "/repo/public/index.html",
        "messages": [
          {
            "ruleId": "element-permitted-content",
            "severity": 2,
            "message": "<button> cannot appear inside <a>",
            "line": 9,
            "column": 4
          },
          {
            "ruleId": "no-trailing-whitespace",
            "severity": 1,
            "message": "warning ignored",
            "line": 2,
            "column": 1
          }
        ]
      }
    ]"#;
    let html_findings = parse_html_validate_json(html_json, root)?;
    assert_eq!(html_findings.len(), 1);
    assert_eq!(html_findings[0].tool, ExternalToolKind::HtmlValidate);
    assert_eq!(html_findings[0].path.as_str(), "public/index.html");
    assert_eq!((html_findings[0].line, html_findings[0].column), (9, 4));
    assert_eq!(html_findings[0].native_code, "element-permitted-content");
    Ok(())
}
