//! Red Team test suite for the Mutation Testing Fortress (`GF052`), inline mutant-skip
//! anti-cheat (`GF070`), `mutants.toml` / `stryker.config.json` config audit (`GF081`),
//! and Git-committed `outcomes.json` fraud blocking (`GF070`).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_engine::{CheckOptions, ScanScope, ingest_mutation_json, run_check_with_options};

static MUT_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_mut_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = MUT_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-mut7-{tag}-{pid}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_rel(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

fn build_cargo_mutants_fixture() -> String {
    let mut items = vec![String::from(
        "{\"summary\":\"Success\",\"scenario\":\"Baseline\"}",
    )];
    for idx in 0..9 {
        items.push(format!(
            "{{\"summary\":\"CaughtMutant\",\"scenario\":{{\"Mutant\":{{\"file\":\"src/calc.rs\",\"function\":{{\"function_name\":\"fn_{idx}\"}},\"span\":{{\"start\":{{\"line\":10}},\"end\":{{\"line\":10}}}},\"replacement\":\"0\",\"genre\":\"FnValue\"}}}}}}"
        ));
    }
    items.push(String::from(
        "{\"summary\":\"Timeout\",\"scenario\":{\"Mutant\":{\"file\":\"src/calc.rs\",\"function\":\"spin\",\"span\":{\"start\":{\"line\":14},\"end\":{\"line\":14}},\"replacement\":\"true\",\"genre\":\"BinaryOperator\"}}}",
    ));
    for line in [18, 19] {
        items.push(format!(
            "{{\"summary\":\"Unviable\",\"scenario\":{{\"Mutant\":{{\"file\":\"src/calc.rs\",\"span\":{{\"start\":{{\"line\":{line}}}}},\"replacement\":\"()\",\"genre\":\"FnValue\"}}}}}}"
        ));
    }
    for (line, rep) in [(27, ">="), (34, "false")] {
        items.push(format!(
            "{{\"summary\":\"MissedMutant\",\"scenario\":{{\"Mutant\":{{\"file\":\"src/calc.rs\",\"function\":{{\"function_name\":\"check_limit\"}},\"span\":{{\"start\":{{\"line\":{line}}},\"end\":{{\"line\":{line}}}}},\"replacement\":\"{rep}\",\"genre\":\"BinaryOperator\"}}}}}}"
        ));
    }
    format!("{{\"outcomes\":[{}]}}", items.join(","))
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn scenario_1_and_3_cargo_mutants_83_pct_and_vacuous_zero_viable_fail_gf052() -> TestResult {
    let dir = fresh_mut_dir("cargo-mutants")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n\
         [mutation]\nmin_mutation_score = 85\nreport_path = \"mutants.out/outcomes.json\"\n",
    )?;
    write_rel(
        &dir,
        "src/lib.rs",
        "/// Calc.\npub const fn ok(x: u32) -> u32 { x + 1 }\n",
    )?;
    let raw_outcomes = build_cargo_mutants_fixture();
    let summary = ingest_mutation_json(&raw_outcomes, &dir)?;
    assert_eq!(
        (summary.total_viable(), summary.score_percent_floor()),
        (12, Some(83))
    );

    write_rel(&dir, "mutants.out/outcomes.json", &raw_outcomes)?;
    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let gf052: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF052")
        .collect();
    assert_eq!(gf052.len(), 3);
    assert!(gf052.iter().any(|d| d.measured == 83 && d.threshold == 85));
    assert!(
        gf052
            .iter()
            .any(|d| d.path.as_str() == "src/calc.rs" && d.span.start_line() == 27)
    );

    let vacuous = "{\"outcomes\":[{\"summary\":\"Success\",\"scenario\":\"Baseline\"},{\"summary\":\"Unviable\",\"scenario\":{\"Mutant\":{\"file\":\"src/lib.rs\"}}}]}";
    assert_eq!(
        ingest_mutation_json(vacuous, &dir)?.score_percent_floor(),
        None
    );
    write_rel(&dir, "mutants.out/outcomes.json", vacuous)?;
    let vac_rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        vac_rep
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "GF052" && d.message.contains("zero viable mutants"))
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn scenario_2_stryker_no_coverage_in_zero_survivor_glob_fails_even_at_96_pct()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_mut_dir("stryker-zs")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n\
         [mutation]\nmin_mutation_score = 90\nzero_survivor_globs = [\"src/domain/**\"]\n\
         report_path = \"reports/mutation/mutation.json\"\n",
    )?;
    write_rel(
        &dir,
        "src/lib.rs",
        "/// Clean.\npub const fn id(v: u32) -> u32 {\n    v\n}\n",
    )?;

    let mut killed_mutants = String::new();
    for idx in 1..=24 {
        if idx > 1 {
            killed_mutants.push(',');
        }
        write!(
            killed_mutants,
            "{{\"mutatorName\":\"ArithmeticOperator\",\"replacement\":\"-\",\"status\":\"Killed\",\"location\":{{\"start\":{{\"line\":{idx}}},\"end\":{{\"line\":{idx}}}}}}}"
        )?;
    }
    let stryker_json = format!(
        "{{\"files\":{{\"src/util.ts\":{{\"mutants\":[{killed_mutants}]}},\
         \"src/domain/ledger.ts\":{{\"mutants\":[{{\"mutatorName\":\"ConditionalExpression\",\"replacement\":\"false\",\"status\":\"NoCoverage\",\"location\":{{\"start\":{{\"line\":44}},\"end\":{{\"line\":44}}}}}}]}}}}}}"
    );

    let parsed = ingest_mutation_json(&stryker_json, &dir)?;
    assert_eq!(parsed.score_percent_floor(), Some(96));
    write_rel(&dir, "reports/mutation/mutation.json", &stryker_json)?;

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(rep.diagnostics.len(), 1);
    assert_eq!(rep.diagnostics[0].rule_id, "GF052");
    assert_eq!(rep.diagnostics[0].path.as_str(), "src/domain/ledger.ts");
    assert_eq!(rep.diagnostics[0].span.start_line(), 44);

    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn has_diag(rep: &gf_engine::CheckReport, rule_id: &str, path: &str) -> bool {
    rep.diagnostics
        .iter()
        .any(|d| d.rule_id == rule_id && d.path.as_str() == path)
}

#[test]
fn scenario_4_and_5_anti_cheat_blocks_inline_mutant_skips_and_sabotaged_configs() -> TestResult {
    let dir = fresh_mut_dir("anti-cheat")?;
    for (rel, body) in [
        (
            "garde-fou.toml",
            "schema_version = 1\n[tools.python]\nruff = \"off\"\nmypy = \"off\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
        ),
        (
            "src/lib.rs",
            "/// Skip1.\n#[mutants::skip]\npub const fn a() -> u32 { 1 }\n\
             /// Skip2.\n#[cfg_attr(test, mutants::skip)]\npub const fn b() -> u32 { 2 }\n",
        ),
        (
            "src/app.ts",
            "// Stryker disable next-line\nexport const rate = (x: number): number => x * 2;\n",
        ),
        (
            "src/calc.py",
            "\"\"\"Calc.\"\"\"\n\ndef add(a: int, b: int) -> int:\n    return a + b  # pragma: no mutate\n",
        ),
        ("mutants.toml", "exclude_globs = [\"crates/**\"]\n"),
        (
            "stryker.config.json",
            "{ \"mutate\": [], \"thresholds\": { \"break\": 0 } }\n",
        ),
    ] {
        write_rel(&dir, rel, body)?;
    }

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    for path in ["src/lib.rs", "src/app.ts", "src/calc.py"] {
        assert!(has_diag(&rep, "GF070", path), "expected GF070 on {path}");
    }
    for cfg_path in ["mutants.toml", "stryker.config.json"] {
        assert!(
            has_diag(&rep, "GF081", cfg_path),
            "expected GF081 on {cfg_path}"
        );
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn scenario_6_git_committed_outcomes_json_is_blocked_by_gf070() -> TestResult {
    let dir = fresh_mut_dir("git-fraud")?;
    assert!(
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&dir)
            .status()?
            .success()
    );
    for (rel, body) in [
        (
            "garde-fou.toml",
            "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
        ),
        (
            "src/lib.rs",
            "/// Clean.\npub const fn ok() -> u32 {\n    1\n}\n",
        ),
        (
            "outcomes.json",
            "{\"outcomes\":[{\"summary\":\"CaughtMutant\",\"scenario\":{\"Mutant\":{\"file\":\"src/lib.rs\"}}}]}",
        ),
    ] {
        write_rel(&dir, rel, body)?;
    }
    assert!(
        Command::new("git")
            .args(["add", "-f", "outcomes.json"])
            .current_dir(&dir)
            .status()?
            .success()
    );

    let mut opts = CheckOptions::new(ScanScope::All, true);
    opts.mutants_path = Some(String::from("outcomes.json"));
    let rep = run_check_with_options(&dir, &opts)?;
    assert!(has_diag(&rep, "GF070", "outcomes.json"));
    fs::remove_dir_all(&dir)?;
    Ok(())
}
