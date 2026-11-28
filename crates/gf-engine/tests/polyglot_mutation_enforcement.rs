//! Integration tests for polyglot mutation-testing enforcement (`GF052` / `GF081`)
//! across Rust, TypeScript, Python, Go, Kotlin, C#, PHP, and Dart.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{
    Config, FeatureId, LanguageId, ProjectContext, ScannerPolicy, WorkspaceArchetype,
    compose_agents_managed_block, mutation_stack_for, supports_mutation_testing,
};
use gf_engine::{ScanScope, ingest_mutation_json, run_check};

static SEQ: AtomicU64 = AtomicU64::new(0);
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fresh_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-mut-poly-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn mutation_stack_covers_all_eight_programming_languages_and_excludes_non_code_dsls() {
    let expected = [
        LanguageId::Rust,
        LanguageId::TypeScript,
        LanguageId::Python,
        LanguageId::Go,
        LanguageId::Kotlin,
        LanguageId::CSharp,
        LanguageId::Php,
        LanguageId::Dart,
    ];
    for lang in expected {
        assert!(supports_mutation_testing(lang), "expected {lang:?}");
        let spec = mutation_stack_for(lang);
        assert!(spec.is_some());
        if let Some(s) = spec {
            assert!(!s.tool.is_empty() && !s.command.is_empty() && !s.report_path.is_empty());
        }
    }
    for non_code in [
        LanguageId::Markdown,
        LanguageId::Yaml,
        LanguageId::Toml,
        LanguageId::Css,
        LanguageId::Sql,
    ] {
        assert!(!supports_mutation_testing(non_code));
    }
}

#[test]
fn ingest_gremlins_mutmut_and_pitest_reports_accurately() -> TestResult {
    let ws = fresh_ws("ingest")?;
    let gremlins = r#"{
        "go_module": "example.com/app",
        "files": [{
            "file_name": "pkg/math.go",
            "mutations": [
                {"line": 10, "type": "ARITHMETIC_BASE", "status": "KILLED"},
                {"line": 12, "type": "CONDITIONALS_BOUNDARY", "status": "TIMED_OUT"},
                {"line": 15, "type": "CONDITIONALS_NEGATION", "status": "LIVED"},
                {"line": 18, "type": "INCREMENT_DECREMENT", "status": "NOT_VIABLE"}
            ]
        }]
    }"#;
    let g_sum = ingest_mutation_json(gremlins, &ws)?;
    assert_eq!(g_sum.caught, 1);
    assert_eq!(g_sum.timeout, 1);
    assert_eq!(g_sum.missed, 1);
    assert_eq!(g_sum.unviable, 1);
    assert_eq!(g_sum.score_percent_floor(), Some(66));
    assert_eq!(g_sum.survivors.len(), 1);
    assert_eq!(g_sum.survivors[0].path.as_str(), "pkg/math.go");

    let mutmut = r#"{
        "mutants": [
            {"filename": "src/calc.py", "line_number": 4, "status": "killed"},
            {"filename": "src/calc.py", "line_number": 9, "status": "survived", "mutator": "NumberReplacer", "replacement": "x + 2", "function_name": "add"},
            {"filename": "src/calc.py", "line_number": 12, "status": "skipped"}
        ]
    }"#;
    let m_sum = ingest_mutation_json(mutmut, &ws)?;
    assert_eq!(m_sum.caught, 1);
    assert_eq!(m_sum.missed, 1);
    assert_eq!(m_sum.unviable, 1);
    assert_eq!(m_sum.score_percent_floor(), Some(50));
    assert_eq!(m_sum.survivors[0].function_name.as_deref(), Some("add"));

    let pitest = r#"<mutations>
        <mutation detected="true" status="KILLED"><sourceFile>src/Main.kt</sourceFile><mutatedMethod>sum</mutatedMethod><lineNumber>5</lineNumber><mutator>org.pitest.MathMutator</mutator><description>Replaced + with -</description></mutation>
        <mutation detected="false" status="SURVIVED"><sourceFile>src/Main.kt</sourceFile><mutatedMethod>mul</mutatedMethod><lineNumber>11</lineNumber><mutator>org.pitest.MathMutator</mutator><description>Replaced * with /</description></mutation>
    </mutations>"#;
    let p_sum = ingest_mutation_json(pitest, &ws)?;
    assert_eq!(p_sum.caught, 1);
    assert_eq!(p_sum.missed, 1);
    assert_eq!(p_sum.score_percent_floor(), Some(50));
    assert_eq!(p_sum.survivors[0].mutator, "MathMutator");
    fs::remove_dir_all(&ws)?;
    Ok(())
}

#[test]
fn stryker_net_and_php_infection_config_weakening_triggers_gf081() -> TestResult {
    let ws = fresh_ws("cfg")?;
    let mut cfg = Config::strict_defaults();
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;

    fs::write(
        ws.join("stryker-config.json"),
        r#"{"stryker-config": {"mutate": [], "thresholds": {"break": 0}}}"#,
    )?;
    fs::write(
        ws.join("infection.json5"),
        r#"{"source": {"directories": []}, "minMsi": 0}"#,
    )?;

    let report = run_check(&ws, ScanScope::All, &cfg)?;
    let stryker_hits = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF081" && d.path.as_str() == "stryker-config.json")
        .count();
    let infection_hits = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF081" && d.path.as_str() == "infection.json5")
        .count();
    assert_eq!(stryker_hits, 2);
    assert_eq!(infection_hits, 2);
    fs::remove_dir_all(&ws)?;
    Ok(())
}

#[test]
fn agents_md_and_init_templates_enforce_mutation_stack_for_relevant_languages() -> TestResult {
    let langs = BTreeSet::from([LanguageId::Python, LanguageId::Go, LanguageId::Kotlin]);
    let feats = BTreeSet::from([FeatureId::TaskRunner]);
    let ctx = ProjectContext {
        archetype: WorkspaceArchetype::Software,
        languages: langs.clone(),
        features: feats.clone(),
        frameworks: BTreeSet::new(),
        dockerignore_entries: None,
    };
    let agents = compose_agents_managed_block(&ctx, &Config::strict_defaults(), &[]);
    assert!(agents.contains("## Mutation Testing Stack (`GF052`)"));
    assert!(agents.contains("mutmut"));
    assert!(agents.contains("gremlins"));
    assert!(agents.contains("PITest"));

    let ws = fresh_ws("init")?;
    let req = gf_engine::InitRequest {
        root: ws.clone(),
        project_name: Some(String::from("poly-app")),
        archetype: Some(WorkspaceArchetype::Software),
        languages: langs,
        features: feats,
        frameworks: BTreeSet::new(),
        run_git_init: false,
    };
    gf_engine::run_init(&req)?;
    let toml_raw = fs::read_to_string(ws.join("garde-fou.toml"))?;
    assert!(toml_raw.contains("min_mutation_score = 100"));
    assert!(toml_raw.contains("garde check --mutants"));
    let taskfile_raw = fs::read_to_string(ws.join("Taskfile.yml"))?;
    assert!(taskfile_raw.contains("mutate:"));
    assert!(taskfile_raw.contains("mutmut"));
    assert!(taskfile_raw.contains("gremlins"));
    assert!(taskfile_raw.contains("./gradlew pitest"));
    fs::remove_dir_all(&ws)?;
    Ok(())
}
