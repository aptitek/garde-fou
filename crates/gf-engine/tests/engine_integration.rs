//! Integration tests for strict TOML config, file discovery, Git `--staged`, and `BLAKE3` cache.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, RepoPathError, SpanError};
use gf_engine::{
    CheckOptions, EngineError, InitOutcome, ScanScope, run_check, run_check_with_options,
    scaffold_config,
};
use gf_parsers::ParseError;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn create_temp_workspace(prefix: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-engine-{prefix}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn rejects_unknown_fields_in_garde_fou_toml() -> Result<(), Box<dyn std::error::Error>> {
    let dir = create_temp_workspace("bad-toml")?;
    fs::write(
        dir.join("garde-fou.toml"),
        "schema_version = 1\n[limits]\nmax_typo_lines = 999\n",
    )?;
    let res = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true));
    assert!(matches!(res, Err(EngineError::InvalidConfig(_))));
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn populates_blake3_cache_and_serves_subsequent_hits() -> Result<(), Box<dyn std::error::Error>> {
    let dir = create_temp_workspace("cache")?;
    assert!(matches!(scaffold_config(&dir)?, InitOutcome::Created(_)));
    assert!(matches!(
        scaffold_config(&dir)?,
        InitOutcome::AlreadyPresent(_)
    ));

    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )?;
    fs::write(
        dir.join("src/main.py"),
        "def greet(name):\n    return name\n",
    )?;

    let first = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(
        (first.files_checked, first.cache_hits, first.cache_misses),
        (2, 0, 2)
    );
    assert!(first.is_clean());
    assert!(dir.join(".garde-fou-cache/v2").is_dir());

    let second = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, false))?;
    assert_eq!(
        (second.files_checked, second.cache_hits, second.cache_misses),
        (2, 2, 0)
    );
    assert_eq!(first.cache_fingerprint, second.cache_fingerprint);
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn invalidates_cache_entry_when_source_file_changes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = create_temp_workspace("invalidate")?;
    scaffold_config(&dir)?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("src/a.rs"), "pub const fn ok() -> i32 { 1 }\n")?;
    fs::write(dir.join("src/b.py"), "def ok():\n    return 1\n")?;

    run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    fs::write(dir.join("src/a.rs"), "pub fn broken( {\n")?;

    let invalidated = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, false))?;
    assert_eq!((invalidated.cache_hits, invalidated.cache_misses), (1, 1));
    assert!(!invalidated.is_clean());
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn discovers_only_staged_files_and_respects_ignore_paths() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = create_temp_workspace("staged")?;
    assert!(
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&dir)
            .status()?
            .success()
    );

    fs::write(
        dir.join("garde-fou.toml"),
        "schema_version = 1\n[paths]\nignore_paths = [\"vendor/**\"]\n",
    )?;
    fs::create_dir_all(dir.join("src"))?;
    fs::create_dir_all(dir.join("vendor"))?;
    fs::write(
        dir.join("src/index.ts"),
        "export const ok = (x: number): number => x + 1;\n",
    )?;
    fs::write(
        dir.join("src/unstaged.ts"),
        "export const broken = ( => ;\n",
    )?;
    fs::write(dir.join("vendor/gen.ts"), "export const ignored = ( => ;\n")?;

    assert!(
        Command::new("git")
            .args(["add", "src/index.ts", "vendor/gen.ts"])
            .current_dir(&dir)
            .status()?
            .success()
    );

    let staged_report =
        run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?;
    assert_eq!(staged_report.files_checked, 1);
    assert!(staged_report.is_clean());

    let direct_report = run_check(&dir, ScanScope::StagedOnly, &Config::strict_defaults())?;
    assert_eq!(direct_report.files_checked, 2);
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn formats_engine_error_variants_cleanly() {
    assert_ne!(EngineError::from(RepoPathError::Empty).to_string(), "");
    assert_ne!(
        EngineError::from(ParseError::InvalidSpan(SpanError::ZeroLine)).to_string(),
        ""
    );
    assert_ne!(EngineError::GitCommand(String::from("err")).to_string(), "");
}
