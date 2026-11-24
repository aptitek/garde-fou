//! Automated smoke and security verification for `scripts/install.sh` and `scripts/install.ps1`,
//! satisfying `GF065` mandatory `Scripts` domain test coverage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const fn garde_bin() -> &'static str {
    env!("CARGO_BIN_EXE_garde")
}

fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("cannot resolve workspace root"))
}

#[cfg(unix)]
fn compute_sha256_hex(archive_path: &Path) -> Result<String, std::io::Error> {
    if let Ok(out) = Command::new("sha256sum").arg(archive_path).output()
        && out.status.success()
    {
        return Ok(String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string());
    }
    let shasum = Command::new("shasum")
        .args(["-a", "256"])
        .arg(archive_path)
        .output()?;
    Ok(String::from_utf8_lossy(&shasum.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string())
}

#[cfg(unix)]
struct StagedRelease {
    dist: PathBuf,
    archive_name: String,
}

#[cfg(unix)]
fn stage_release_archive(
    root: &Path,
    work: &Path,
    target: &str,
) -> Result<StagedRelease, Box<dyn std::error::Error>> {
    let stage = work.join("stage");
    let dist = work.join("dist");
    fs::create_dir_all(&stage)?;
    fs::create_dir_all(&dist)?;
    fs::copy(garde_bin(), stage.join("garde"))?;
    fs::copy(root.join("README.md"), stage.join("README.md"))?;

    let archive_name = format!("garde-{target}.tar.gz");
    let archive_path = dist.join(&archive_name);
    let tar_status = Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&stage)
        .args(["garde", "README.md"])
        .status()?;
    assert!(tar_status.success());

    let digest = compute_sha256_hex(&archive_path)?;
    fs::write(
        dist.join("SHA256SUMS"),
        format!("{digest}  {archive_name}\n"),
    )?;
    Ok(StagedRelease { dist, archive_name })
}

#[cfg(unix)]
#[test]
fn posix_install_sh_verifies_sha256_and_rejects_tampered_archives()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let script = root.join("scripts/install.sh");
    assert!(script.is_file());

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let work = std::env::temp_dir().join(format!("gf-installer-smoke-{nanos}"));
    let bin = work.join("bin");
    fs::create_dir_all(&bin)?;

    let target = "x86_64-unknown-linux-musl";
    let staged = stage_release_archive(&root, &work, target)?;

    let ok_install = Command::new("sh")
        .arg(&script)
        .arg("--from-dir")
        .arg(&staged.dist)
        .arg("--install-dir")
        .arg(&bin)
        .env("GARDE_TARGET", target)
        .output()?;
    assert!(ok_install.status.success());

    let ver = Command::new(bin.join("garde")).arg("--version").output()?;
    assert!(ver.status.success());

    fs::write(
        staged.dist.join("SHA256SUMS"),
        format!("{:064}  {}\n", 0, staged.archive_name),
    )?;
    let bad_install = Command::new("sh")
        .arg(&script)
        .arg("--from-dir")
        .arg(&staged.dist)
        .arg("--install-dir")
        .arg(work.join("bin-bad"))
        .env("GARDE_TARGET", target)
        .output()?;
    assert!(!bad_install.status.success());

    fs::remove_dir_all(&work)?;
    Ok(())
}

#[test]
fn powershell_install_ps1_enforces_strict_mode_and_sha256_verification()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let ps1_script = root.join("scripts/install.ps1");
    assert!(ps1_script.is_file());

    let content = fs::read_to_string(&ps1_script)?;
    assert!(content.contains("Set-StrictMode -Version Latest"));
    assert!(content.contains("$ErrorActionPreference = 'Stop'"));
    assert!(content.contains("Get-FileHash"));
    assert!(content.contains("SHA256SUMS"));
    assert!(content.contains("SHA-256 mismatch"));

    if Command::new("pwsh")
        .arg("-Version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        let ast_ok = Command::new("pwsh")
            .args([
                "-NoProfile",
                "-Command",
                "param($p) $errs = $null; $null = [System.Management.Automation.Language.Parser]::ParseFile($p, [ref]$null, [ref]$errs); if ($errs) { exit 1 }",
            ])
            .arg(&ps1_script)
            .status()?;
        assert!(ast_ok.success());
    }
    Ok(())
}
