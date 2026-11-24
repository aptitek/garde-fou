//! Unit and staged-filter tests for `ExternalAdapter`, `strict_tools`, and `uv run --no-sync`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_engine::{
    CheckOptions, ExternalAdapter, ResolvedPythonTool, ScanScope, resolve_python_tool_with_uv,
    run_check_with_options,
};

type TestRes = Result<(), Box<dyn std::error::Error>>;

static RUNNER_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_runner_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = RUNNER_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-py-runner-{tag}-{pid}-{nanos}-{seq}"));
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

#[test]
fn external_adapter_build_args_and_install_hints_are_exact() {
    assert_eq!(
        ExternalAdapter::Ruff.build_args(&[]),
        vec!["check", "--output-format", "json", "--no-fix", "--", "."]
    );
    let files = vec![String::from("src/a.py"), String::from("src/b.py")];
    assert_eq!(
        ExternalAdapter::Mypy.build_args(&files),
        vec![
            "--strict",
            "--output",
            "json",
            "--no-error-summary",
            "--",
            "src/a.py",
            "src/b.py"
        ]
    );
    assert_eq!(
        ExternalAdapter::Pyright.build_args(&[]),
        vec!["--outputjson", "."]
    );
    assert!(ExternalAdapter::Ruff.install_hint().contains("ruff"));
    assert!(ExternalAdapter::Pyright.install_hint().contains("pyright"));
}

#[cfg(unix)]
#[test]
fn resolve_python_tool_uv_no_sync_requires_venv_and_manifest_or_lock() -> TestRes {
    let dir = fresh_runner_dir("uv-probe")?;
    let mock_uv = dir.join("mock-uv");
    write_executable(
        &dir,
        "mock-uv",
        "#!/bin/sh\nif [ \"$1\" = \"run\" ] && [ \"$2\" = \"--no-sync\" ]; then\n  printf '[]'\n  exit 0\nfi\nexit 1\n",
    )?;
    let tool = "gf_synthetic_py_linter";

    write_rel(&dir, "pyproject.toml", "[project]\nname = \"x\"\n")?;
    assert_eq!(resolve_python_tool_with_uv(&dir, tool, &mock_uv), None);

    fs::remove_file(dir.join("pyproject.toml"))?;
    fs::create_dir_all(dir.join(".venv"))?;
    assert_eq!(resolve_python_tool_with_uv(&dir, tool, &mock_uv), None);

    write_rel(&dir, "pyproject.toml", "[project]\nname = \"x\"\n")?;
    let resolved = resolve_python_tool_with_uv(&dir, tool, &mock_uv);
    assert_eq!(
        resolved,
        Some(ResolvedPythonTool::UvNoSync {
            uv_bin: mock_uv.clone(),
            tool: String::from(tool),
        })
    );
    if let Some(launcher) = resolved {
        let out = launcher.execute(&dir, &[String::from(".")])?;
        assert!(out.status.success());
    }

    fs::remove_file(dir.join("pyproject.toml"))?;
    write_rel(&dir, "uv.lock", "version = 1\n")?;
    assert!(matches!(
        resolve_python_tool_with_uv(&dir, tool, &mock_uv),
        Some(ResolvedPythonTool::UvNoSync { .. })
    ));

    fs::remove_dir_all(&dir)?;
    Ok(())
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

#[cfg(unix)]
fn seed_staged_runner_repo(dir: &Path) -> Result<(), std::io::Error> {
    git_ok(dir, &["init", "-b", "main"])?;
    write_rel(
        dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"auto\"\nmypy = \"off\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        dir,
        "pyproject.toml",
        "[project]\nname = \"p\"\n[tool.mypy]\nstrict = true\n",
    )?;
    write_rel(
        dir,
        "src/mod.py",
        "\"\"\"Mod.\"\"\"\n\ndef f(x: int) -> int:\n    return x\n",
    )?;
    write_rel(
        dir,
        "tests/test_mod.py",
        "from mod import f\ndef test_f() -> None:\n    assert f(1) == 1\n",
    )
}

#[cfg(unix)]
#[test]
fn strict_tools_preserves_off_and_staged_py_or_pyproject_invokes_exact_targets() -> TestRes {
    let dir = fresh_runner_dir("staged-targets")?;
    seed_staged_runner_repo(&dir)?;

    let mut strict_opts = CheckOptions::new(ScanScope::All, true);
    strict_opts.strict_tools = true;
    strict_opts.no_cache = true;
    let strict_rep = run_check_with_options(&dir, &strict_opts)?;
    assert!(
        !strict_rep
            .diagnostics
            .iter()
            .filter(|d| d.rule_id == "GF043")
            .any(|d| d.message.contains("mypy") || d.message.contains("pyright"))
    );

    let log_file = dir.join("args.log");
    write_executable(
        &dir,
        ".venv/bin/ruff",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\nprintf '[]'\n",
            log_file.display()
        ),
    )?;

    git_ok(&dir, &["add", "src/mod.py", "tests/test_mod.py"])?;
    assert!(
        run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?.is_clean()
    );
    assert!(fs::read_to_string(&log_file)?.contains("src/mod.py"));

    git_ok(&dir, &["reset"])?;
    fs::remove_file(&log_file)?;
    git_ok(&dir, &["add", "pyproject.toml"])?;
    assert!(
        run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?.is_clean()
    );
    assert!(fs::read_to_string(&log_file)?.trim_end().ends_with(" -- ."));

    fs::remove_dir_all(&dir)?;
    Ok(())
}
