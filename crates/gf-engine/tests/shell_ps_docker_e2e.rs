//! End-to-end workspace integration tests for extensionless `#!` shebang discovery,
//! `[tools.shell]`/`[tools.powershell]`/`[tools.docker]` TOML policies, `ExternalAdapter` argv,
//! and `--strict-tools` `GF043` enforcement.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{ExternalToolKind, LanguageId, ScannerPolicy};
use gf_engine::{
    CheckOptions, ExternalAdapter, ScanScope, load_workspace_config, run_check_with_options,
};

type Res = Result<(), Box<dyn std::error::Error>>;

static E2E_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_e2e_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = E2E_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-sh-ps-e2e-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn put_file(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

fn populate_shebang_workspace(dir: &Path) -> Result<(), std::io::Error> {
    let toml = "\
schema_version = 1
[integrity]
require_lock = false
[agents]
sync_policy = \"off\"
[scanners]
secrets = \"off\"
vulns = \"off\"
[tools.shell]
shellcheck = \"off\"
[tools.powershell]
psscriptanalyzer = \"off\"
[tools.docker]
hadolint = \"off\"
";
    put_file(dir, "garde-fou.toml", toml)?;
    put_file(
        dir,
        "scripts/bootstrap",
        "#!/usr/bin/env sh\nset -eu\nrm $UNQUOTED_BOOT_PATH\n",
    )?;
    put_file(
        dir,
        "scripts/win-task",
        "#!/usr/bin/env pwsh\nSet-StrictMode -Version Latest\n$ErrorActionPreference = 'Stop'\nif ($items -eq $null) { Write-Output 'bad' }\n",
    )?;
    put_file(
        dir,
        "tests/scripts_smoke.rs",
        "#[test]\nfn runs_scripts() {\n    let a = std::process::Command::new(\"sh\").arg(\"scripts/bootstrap\").status();\n    let b = std::process::Command::new(\"pwsh\").arg(\"scripts/win-task\").status();\n    assert!(a.is_ok() || a.is_err());\n    assert!(b.is_ok() || b.is_err());\n    assert_eq!(1 + 1, 2);\n}\n",
    )
}

#[test]
fn extensionless_shebang_scripts_and_tools_toml_sections_are_analyzed_end_to_end() -> Res {
    let dir = fresh_e2e_dir("shebang")?;
    populate_shebang_workspace(&dir)?;

    let loaded = load_workspace_config(&dir)?;
    assert_eq!(loaded.analyzer.tools.shell.shellcheck, ScannerPolicy::Off);
    assert_eq!(
        loaded.analyzer.tools.powershell.psscriptanalyzer,
        ScannerPolicy::Off
    );
    assert_eq!(loaded.analyzer.tools.docker.hadolint, ScannerPolicy::Off);

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(report.context.languages.contains(&LanguageId::Shell));
    assert!(report.context.languages.contains(&LanguageId::PowerShell));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path.as_str() == "scripts/bootstrap" && d.message.contains("[SH-SC2086]")),
        "got {:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path.as_str() == "scripts/win-task" && d.message.contains("[PS-NULL-LEFT]")),
        "got {:?}",
        report.diagnostics
    );

    put_file(
        &dir,
        "garde-fou.toml",
        "[tools.shell]\nunknown_key = \"auto\"\n",
    )?;
    assert!(load_workspace_config(&dir).is_err());
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn external_adapter_argv_parsers_and_required_policy_gf043_work_without_shell() -> Res {
    let targets = vec![String::from("scripts/a.sh"), String::from("scripts/b.sh")];
    assert_eq!(
        ExternalAdapter::Shellcheck.build_args(&targets),
        vec!["--format=json1", "--", "scripts/a.sh", "scripts/b.sh"]
    );
    assert_eq!(
        ExternalAdapter::Hadolint.build_args(&[String::from("Dockerfile")]),
        vec!["--format", "json", "--no-color", "Dockerfile"]
    );
    let pssa_args = ExternalAdapter::PsScriptAnalyzer.build_args(&[String::from("scripts/x.ps1")]);
    assert_eq!(pssa_args[0], "-NoLogo");
    assert_eq!(pssa_args.last().map(String::as_str), Some("scripts/x.ps1"));
    assert_eq!(
        ExternalAdapter::Shellcheck.tool_kind(),
        ExternalToolKind::Shellcheck
    );
    assert_eq!(
        ExternalAdapter::Hadolint.tool_kind(),
        ExternalToolKind::Hadolint
    );
    assert_eq!(
        ExternalAdapter::PsScriptAnalyzer.tool_kind(),
        ExternalToolKind::PsScriptAnalyzer
    );

    let dir = fresh_e2e_dir("req-gf043")?;
    put_file(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[integrity]\nrequire_lock = false\n[agents]\nsync_policy = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n[tools.docker]\nhadolint = \"required\"\n",
    )?;
    put_file(&dir, ".dockerignore", ".git\n")?;
    put_file(
        &dir,
        "Dockerfile",
        "FROM debian:12.8-slim\nWORKDIR /app\nUSER appuser\nCMD [\"echo\", \"ok\"]\n",
    )?;

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.rule_id == "GF043" && d.message.contains("hadolint")),
        "got {:?}",
        rep.diagnostics
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}
