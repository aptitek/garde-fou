//! End-to-end smoke tests exercising the compiled `garde-fou` binary.

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const fn garde_fou_bin() -> &'static str {
    env!("CARGO_BIN_EXE_garde-fou")
}

#[test]
fn cli_help_and_version_succeed() -> Result<(), Box<dyn std::error::Error>> {
    let help = Command::new(garde_fou_bin()).arg("--help").output()?;
    assert!(help.status.success());
    let help_stdout = String::from_utf8(help.stdout)?;
    assert!(help_stdout.contains("garde-fou"));
    assert!(help_stdout.contains("check"));
    assert!(help_stdout.contains("init"));

    let version = Command::new(garde_fou_bin()).arg("--version").output()?;
    assert!(version.status.success());
    let version_stdout = String::from_utf8(version.stdout)?;
    assert!(version_stdout.contains("garde-fou"));
    Ok(())
}

#[test]
fn cli_init_and_check_formats_succeed() -> Result<(), Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir().join(format!("gf-cli-smoke-{nanos}"));
    fs::create_dir_all(&dir)?;

    let init = Command::new(garde_fou_bin())
        .arg("init")
        .current_dir(&dir)
        .output()?;
    assert!(init.status.success());
    assert!(dir.join("garde-fou.toml").is_file());

    let agent = Command::new(garde_fou_bin())
        .args(["check", "--staged", "--format", "agent"])
        .current_dir(&dir)
        .output()?;
    assert!(agent.status.success());
    assert!(String::from_utf8(agent.stdout)?.contains("## Garde-fou Report"));

    let sarif = Command::new(garde_fou_bin())
        .args(["check", "--format", "sarif"])
        .current_dir(&dir)
        .output()?;
    assert!(sarif.status.success());
    assert!(String::from_utf8(sarif.stdout)?.contains(r#""version":"2.1.0""#));

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
