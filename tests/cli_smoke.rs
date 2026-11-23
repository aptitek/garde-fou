//! End-to-end smoke tests exercising the compiled `garde` binary.

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const fn garde_bin() -> &'static str {
    env!("CARGO_BIN_EXE_garde")
}

#[test]
fn cli_help_and_version_succeed() -> Result<(), Box<dyn std::error::Error>> {
    let help = Command::new(garde_bin()).arg("--help").output()?;
    assert!(help.status.success());
    let help_stdout = String::from_utf8(help.stdout)?;
    assert!(help_stdout.contains("garde"));
    assert!(help_stdout.contains("check"));
    assert!(help_stdout.contains("init"));

    let version = Command::new(garde_bin()).arg("--version").output()?;
    assert!(version.status.success());
    let version_stdout = String::from_utf8(version.stdout)?;
    assert!(version_stdout.contains("garde"));
    Ok(())
}

#[test]
fn cli_init_no_cache_and_cache_hit_succeed() -> Result<(), Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir().join(format!("gf-cli-smoke-{nanos}"));
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/lib.rs"),
        "/// Clean fn.\npub const fn id(x: u32) -> u32 {\n    x\n}\n",
    )?;

    let init = Command::new(garde_bin())
        .arg("init")
        .current_dir(&dir)
        .output()?;
    assert!(init.status.success());
    assert!(dir.join("garde-fou.toml").is_file());
    assert!(dir.join(".gitattributes").is_file());

    let fresh = Command::new(garde_bin())
        .args(["check", "--no-cache", "--format", "agent"])
        .current_dir(&dir)
        .output()?;
    assert!(fresh.status.success());
    let fresh_out = String::from_utf8(fresh.stdout)?;
    assert!(fresh_out.contains("`0` hits, `2` misses"));

    let cached = Command::new(garde_bin())
        .args(["check", "--format", "agent"])
        .current_dir(&dir)
        .output()?;
    assert!(cached.status.success());
    let cached_out = String::from_utf8(cached.stdout)?;
    assert!(cached_out.contains("`2` hits, `0` misses"));

    let sarif = Command::new(garde_bin())
        .args(["check", "--format", "sarif"])
        .current_dir(&dir)
        .output()?;
    assert!(sarif.status.success());
    assert!(String::from_utf8(sarif.stdout)?.contains(r#""version":"2.1.0""#));

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
