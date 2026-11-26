//! End-to-end integration tests for Step 8's 3-tier ecosystem tool ingestion pipeline:
//! Tier 1 (`GF080` native Python AST), Tier 2 (`GF081` tool config audit),
//! Tier 3 (`GF082` / `GF043` `.venv` runners & `--staged` fast-path),
//! and the Python + `FastAPI` + Docker Golden Invariant.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{FeatureId, FrameworkId, LanguageId};
use gf_engine::{
    CheckOptions, CheckReport, InitRequest, ScanScope, run_check_with_options, run_init,
};

static PY_SEQ: AtomicU64 = AtomicU64::new(0);

const DIRTY_SERVICE_PY: &str = r#"import pickle
import subprocess
from typing import Any


def process_batch(item: Any, cmd: str, raw: bytes, items=[]):
    print("debug")
    try:
        eval(cmd)
        pickle.loads(raw)
        subprocess.run(cmd, shell=True)
    except ValueError:
        pass
    try:
        items.append(item)
    except:
        pass
    return items
"#;

fn fresh_py_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = PY_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-py-ingest-{tag}-{pid}-{nanos}-{seq}"));
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

#[cfg(unix)]
fn install_mock_venv_tools(dir: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt as _;
    let scripts = [
        (
            ".venv/bin/ruff",
            "#!/bin/sh\nprintf '[{\"code\":\"F841\",\"message\":\"Local variable unused\",\"filename\":\"src/app.py\",\"location\":{\"row\":3,\"column\":5},\"fix\":null}]'\n",
        ),
        (
            ".venv/bin/mypy",
            "#!/bin/sh\nprintf '{\"file\":\"src/app.py\",\"line\":3,\"column\":1,\"message\":\"Missing return\",\"hint\":\"Add return int\",\"code\":\"return\",\"severity\":\"error\"}\\n'\n",
        ),
        (
            ".venv/bin/pyright",
            "#!/bin/sh\nprintf '{\"generalDiagnostics\":[{\"file\":\"src/app.py\",\"severity\":\"error\",\"message\":\"Type mismatch\",\"rule\":\"reportReturnType\",\"range\":{\"start\":{\"line\":2,\"character\":0}}}]}'\n",
        ),
    ];
    for (rel, body) in scripts {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full, body)?;
        let mut perms = fs::metadata(&full)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&full, perms)?;
    }
    Ok(())
}

fn rule_msgs<'a>(report: &'a CheckReport, rule_id: &str) -> Vec<&'a str> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == rule_id)
        .map(|d| d.message.as_str())
        .collect()
}

#[test]
fn tier1_python_ast_guardrails_trigger_gf080_and_exempt_declared_cli_entrypoints()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_py_dir("tier1")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"off\"\nmypy = \"off\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n[paths]\nentrypoints = [\"src/cli.py\"]\n",
    )?;
    write_rel(&dir, "src/service.py", DIRTY_SERVICE_PY)?;
    write_rel(
        &dir,
        "src/cli.py",
        "def run_cli() -> None:\n    print(\"ok\")\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let gf080_msgs = rule_msgs(&report, "GF080");
    for code in [
        "[PY-B006]",
        "[PY-ANN401]",
        "[PY-S102]",
        "[PY-S301]",
        "[PY-S602]",
        "[PY-T201]",
        "[PY-E722]",
        "[PY-B001]",
    ] {
        assert!(gf080_msgs.iter().any(|m| m.contains(code)));
    }
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.path.as_str() == "src/cli.py" && d.rule_id == "GF080")
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn tier2_audits_pyproject_ruff_and_jsonc_tsconfig_under_gf081()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_py_dir("tier2")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"off\"\nmypy = \"off\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"svc\"\nversion = \"0.1.0\"\n[tool.mypy]\nstrict = false\nignore_errors = true\ndisallow_untyped_defs = false\n[tool.ruff.lint]\nignore = [\"ALL\"]\nselect = []\n",
    )?;
    write_rel(&dir, ".ruff.toml", "[lint]\nignore = [\"*\"]\n")?;
    write_rel(
        &dir,
        "tsconfig.json",
        "{\n  // JSONC line comment\n  /* block comment */\n  \"compilerOptions\": {\n    \"strict\": false,\n    \"noImplicitAny\": false,\n  },\n}\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let gf081 = rule_msgs(&report, "GF081");
    for expected in [
        "[mypy:tool.mypy.strict]",
        "[mypy:tool.mypy.ignore_errors]",
        "[mypy:tool.mypy.disallow_untyped_defs]",
        "[ruff:tool.ruff.ignore]",
        "[ruff:tool.ruff.select]",
        "[ruff:ignore]",
        "[tsc:compilerOptions.strict]",
        "[tsc:compilerOptions.noImplicitAny]",
    ] {
        assert!(gf081.iter().any(|m| m.contains(expected)));
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[cfg(unix)]
fn run_git_ok(dir: &Path, args: &[&str]) -> Result<(), std::io::Error> {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()?
            .success()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn tier3_venv_mock_binaries_emit_gf082_and_missing_required_emits_gf043()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_py_dir("tier3-venv")?;
    run_git_ok(&dir, &["init", "-b", "main"])?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"required\"\nmypy = \"required\"\npyright = \"required\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/app.py",
        "\"\"\"Clean.\"\"\"\n\ndef add(a: int, b: int) -> int:\n    return a + b\n",
    )?;
    write_rel(&dir, "docs/guide.md", "# Guide\n\nClean prose.\n")?;

    let missing = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(missing.diagnostics.iter().any(|d| d.rule_id == "GF043"));

    install_mock_venv_tools(&dir)?;
    let full = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let gf082_count = full
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF082")
        .count();
    assert_eq!(gf082_count, 3);

    run_git_ok(&dir, &["add", "docs/guide.md"])?;
    let staged_md = run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?;
    assert!(!staged_md.diagnostics.iter().any(|d| d.rule_id == "GF082"));

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn golden_invariant_python_fastapi_docker_passes_all_three_tiers()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_py_dir("golden-py")?;
    run_init(&InitRequest {
        root: dir.clone(),
        project_name: Some(String::from("payments-api")),
        archetype: None,
        languages: BTreeSet::from([LanguageId::Python]),
        features: BTreeSet::from([FeatureId::Docker]),
        frameworks: BTreeSet::from([FrameworkId::FastApi]),
        run_git_init: true,
    })?;
    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        report.is_clean(),
        "expected 0 violations, got {:#?}",
        report.diagnostics
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}
