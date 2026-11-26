//! Stage 4 end-to-end Python `.venv` mock orchestration, Windows `C:\` + `\r\n` normalization,
//! `--staged` sentinel short-circuit proof, and Golden Invariant (`init --offline` -> `check == 0`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{ExternalToolKind, FeatureId, FrameworkId, LanguageId};
use gf_engine::{
    CheckOptions, InitRequest, ScanScope, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json,
    run_check_with_options, run_init,
};

type TestRes = Result<(), Box<dyn std::error::Error>>;

static EXT_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_e2e_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = EXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-py-e2e-{tag}-{pid}-{nanos}-{seq}"));
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
fn write_executable(root: &Path, rel: &str, script: &str) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt as _;
    write_rel(root, rel, script)?;
    let full = root.join(rel);
    let mut perms = fs::metadata(&full)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&full, perms)
}

#[cfg(unix)]
fn git_ok(dir: &Path, args: &[&str]) -> Result<(), std::io::Error> {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()?
            .success()
    );
    Ok(())
}

#[test]
fn windows_drive_paths_and_crlf_normalize_cleanly_across_all_three_parsers() -> TestRes {
    let win_root = Path::new("C:\\repo");
    let ruff_crlf = "[\r\n  {\"code\":\"F401\",\"message\":\"unused os\",\"filename\":\"C:\\\\repo\\\\src\\\\mod.py\",\"location\":{\"row\":7,\"column\":3},\"fix\":{\"message\":\"Remove unused `os`\"}}\r\n]\r\n";
    let ruff = parse_ruff_json(ruff_crlf, win_root)?;
    assert_eq!(ruff.len(), 1);
    assert_eq!(ruff[0].tool, ExternalToolKind::Ruff);
    assert_eq!(ruff[0].path.as_str(), "src/mod.py");
    assert_eq!((ruff[0].line, ruff[0].column), (7, 3));
    assert_eq!(ruff[0].remediation, "Remove unused `os`");

    let mypy_crlf = "{\"file\":\"C:\\\\repo\\\\src\\\\mod.py\",\"line\":9,\"column\":4,\"message\":\"Incompatible return\",\"hint\":\"Return str instead\",\"code\":\"return-value\",\"severity\":\"error\"}\r\n";
    let mypy = parse_mypy_ndjson(mypy_crlf, win_root)?;
    assert_eq!(mypy.len(), 1);
    assert_eq!(mypy[0].path.as_str(), "src/mod.py");
    assert_eq!(mypy[0].remediation, "Mypy hint: Return str instead");

    let pyright_crlf = "{\r\n\"generalDiagnostics\":[{\"file\":\"file:///C:/repo/src/mod.py\",\"severity\":\"error\",\"message\":\"Bad arg\",\"rule\":\"reportArgumentType\",\"range\":{\"start\":{\"line\":4,\"character\":2}}}]\r\n}\r\n";
    let pyright = parse_pyright_json(pyright_crlf, win_root)?;
    assert_eq!(pyright.len(), 1);
    assert_eq!(pyright[0].path.as_str(), "src/mod.py");
    assert_eq!((pyright[0].line, pyright[0].column), (5, 3));
    Ok(())
}

#[cfg(unix)]
fn install_ruff_and_mypy_sentinel_mocks(dir: &Path, sentinel: &Path) -> Result<(), std::io::Error> {
    let ruff_sh = format!(
        "#!/bin/sh\necho ruff >> \"{}\"\nprintf '[{{\"code\":\"B009\",\"message\":\"getattr constant\",\"filename\":\"src/mod.py\",\"location\":{{\"row\":4,\"column\":5}},\"fix\":{{\"message\":\"Use attribute access\"}}}}]'\n",
        sentinel.display()
    );
    let mypy_sh = format!(
        "#!/bin/sh\necho mypy >> \"{}\"\nprintf '{{\"file\":\"src/mod.py\",\"line\":4,\"column\":12,\"message\":\"Incompatible types\",\"hint\":\"Convert operand to int\",\"code\":\"operator\",\"severity\":\"error\"}}\\n'\n",
        sentinel.display()
    );
    write_executable(dir, ".venv/bin/ruff", &ruff_sh)?;
    write_executable(dir, ".venv/bin/mypy", &mypy_sh)
}

#[cfg(unix)]
fn assert_ingested_ruff_and_mypy(full: &gf_engine::CheckReport) {
    let gf082: Vec<_> = full
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF082")
        .collect();
    assert_eq!(gf082.len(), 2);
    assert!(
        gf082
            .iter()
            .any(|d| d.message.contains("[ruff:B009]") && d.span.start_line() == 4)
    );
    assert!(gf082.iter().any(|d| {
        d.message.contains("[mypy:operator]")
            && d.remediation.contains("Mypy hint: Convert operand to int")
            && d.span.start_line() == 4
    }));
}

#[cfg(unix)]
#[test]
fn mock_venv_binaries_ingest_hints_emit_gf043_when_missing_and_short_circuit_staged_readme()
-> TestRes {
    let dir = fresh_e2e_dir("venv-staged")?;
    git_ok(&dir, &["init", "-b", "main"])?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"required\"\nmypy = \"required\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/mod.py",
        "\"\"\"Module.\"\"\"\n\ndef calc(x: int) -> int:\n    return x + 1\n",
    )?;
    write_rel(&dir, "README.md", "# Project\n\nOverview.\n")?;

    let missing_mypy = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(missing_mypy.diagnostics.iter().any(|d| {
        d.rule_id == "GF043"
            && d.message.contains("mypy")
            && d.remediation.contains("uv add --dev mypy")
    }));

    let sentinel = dir.join("invoked.sentinel");
    install_ruff_and_mypy_sentinel_mocks(&dir, &sentinel)?;

    git_ok(&dir, &["add", "README.md"])?;
    let staged = run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?;
    assert!(staged.is_clean());
    assert!(
        !sentinel.exists(),
        "external Python binaries must NOT be invoked when only README.md is staged"
    );

    let full = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(sentinel.is_file());
    assert_ingested_ruff_and_mypy(&full);

    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn scaffold_and_check_python_variant(
    slug: &str,
    features: BTreeSet<FeatureId>,
    frameworks: BTreeSet<FrameworkId>,
) -> Result<Vec<gf_core::Diagnostic>, Box<dyn std::error::Error>> {
    let dir = fresh_e2e_dir(slug)?;
    run_init(&InitRequest {
        root: dir.clone(),
        project_name: Some(String::from(slug)),
        archetype: None,
        languages: BTreeSet::from([LanguageId::Python]),
        features,
        frameworks,
        run_git_init: true,
    })?;
    let existing_toml = fs::read_to_string(dir.join("garde-fou.toml"))?;
    assert!(existing_toml.contains("[tools.python]"));

    let pyproject = fs::read_to_string(dir.join("pyproject.toml"))?;
    assert!(pyproject.contains("strict = true"));
    assert!(pyproject.contains("[tool.pytest.ini_options]"));
    if slug == "py-fastapi-docker-tdd" {
        let main_py = fs::read_to_string(dir.join("app/main.py"))?;
        assert!(main_py.contains("def healthcheck() -> dict[str, str]:"));
    }
    if slug == "py-django" {
        let views_py = fs::read_to_string(dir.join("src/py_django/views.py"))?;
        assert!(views_py.contains("def status_view() -> dict[str, str]:"));
    }

    let mut check_opts = CheckOptions::new(ScanScope::All, true);
    check_opts.no_cache = true;
    let report = run_check_with_options(&dir, &check_opts)?;
    fs::remove_dir_all(&dir)?;
    Ok(report.diagnostics)
}

#[test]
fn golden_invariant_init_offline_python_plain_fastapi_docker_tdd_and_django() -> TestRes {
    let cases = [
        ("py-plain", BTreeSet::new(), BTreeSet::new()),
        (
            "py-fastapi-docker-tdd",
            BTreeSet::from([FeatureId::Docker, FeatureId::Tdd]),
            BTreeSet::from([FrameworkId::FastApi]),
        ),
        (
            "py-django",
            BTreeSet::new(),
            BTreeSet::from([FrameworkId::Django]),
        ),
    ];
    for (slug, features, frameworks) in cases {
        let diagnostics = scaffold_and_check_python_variant(slug, features, frameworks)?;
        assert_eq!(diagnostics, Vec::new(), "failed for `{slug}`");
    }
    Ok(())
}
