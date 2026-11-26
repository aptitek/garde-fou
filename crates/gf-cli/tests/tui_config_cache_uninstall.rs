//! End-to-end integration tests for lossless `ConfigTab` patching, `garde cache`,
//! surgical `garde uninstall` / `garde eject`, `WatchPoller`/`diff_snapshots`, and 3-tab `AppModel`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use gf_cli::tui::{ConfigTabAction, ConfigTabModel, Key};
use gf_cli::ui::ColorPolicy;
use gf_cli::{Cli, execute_with_policy};
use gf_engine::{
    SkillsFetchMode, apply_project_settings, capture_snapshot, detect_mise_tool_slug,
    diff_snapshots, load_project_settings, load_workspace_config, plan_self_uninstall,
};

fn fresh_temp_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir().join(format!("gf-int-{tag}-{nanos}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn init_offline_workspace(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from([
        "garde",
        "init",
        "--name",
        "demo",
        "--lang",
        "rust",
        "--offline",
        "-y",
    ])?;
    let (_, code) = execute_with_policy(&cli, dir, ColorPolicy::Plain)?;
    assert_eq!(code, ExitCode::SUCCESS);
    Ok(())
}

#[test]
fn lossless_config_patcher_preserves_comments_and_syncs_cleanly()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("cfg")?;
    init_offline_workspace(&dir)?;
    let toml_path = dir.join("garde-fou.toml");
    let orig = fs::read_to_string(&toml_path)?;
    let with_ignore = orig.replacen("ignore_paths = []", "ignore_paths = [\"challenges/**\"]", 1);
    let enriched = format!("# Mon commentaire précieux\n{with_ignore}");
    fs::write(&toml_path, enriched)?;

    let loaded = load_project_settings(&dir)?;
    let mut tab = ConfigTabModel::new(loaded);
    tab.handle_key(Key::Down);
    tab.handle_key(Key::Char(' '));
    tab.handle_key(Key::Right);
    tab.handle_key(Key::Down);
    tab.handle_key(Key::Char('x'));
    assert!(tab.unsaved_changes() >= 1);

    let ConfigTabAction::SaveAndSync(draft) = tab.handle_key(Key::Char('s')) else {
        return Err("expected SaveAndSync".into());
    };
    apply_project_settings(&dir, &draft, SkillsFetchMode::Offline)?;
    let patched = fs::read_to_string(&toml_path)?;
    assert!(patched.contains("# Mon commentaire précieux"));
    assert!(patched.contains("challenges/**"));
    assert!(dir.join("garde.lock").is_file());
    assert!(dir.join("AGENTS.md").is_file());

    let check = Cli::try_parse_from(["garde", "check", "--no-cache"])?;
    let (_, code) = execute_with_policy(&check, &dir, ColorPolicy::Plain)?;
    assert_eq!(code, ExitCode::SUCCESS);
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn cache_cli_status_prune_clean_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("cache")?;
    init_offline_workspace(&dir)?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/lib.rs"),
        "/// Demo.\npub fn ok() -> u8 { 1 }\n",
    )?;

    let chk = Cli::try_parse_from(["garde", "check"])?;
    let (_, code) = execute_with_policy(&chk, &dir, ColorPolicy::Plain)?;
    assert_eq!(code, ExitCode::SUCCESS);

    let stale_dir = dir.join(".garde-fou-cache/v1");
    fs::create_dir_all(&stale_dir)?;
    fs::write(stale_dir.join("old.bin"), b"obsolete-schema-payload")?;

    let st = Cli::try_parse_from(["garde", "cache", "status"])?;
    let (st_out, st_code) = execute_with_policy(&st, &dir, ColorPolicy::Plain)?;
    assert_eq!(st_code, ExitCode::SUCCESS);
    assert!(st_out.contains("v1"));

    let pr = Cli::try_parse_from(["garde", "cache", "prune"])?;
    let (pr_out, pr_code) = execute_with_policy(&pr, &dir, ColorPolicy::Plain)?;
    assert_eq!(pr_code, ExitCode::SUCCESS);
    assert!(pr_out.contains("cache prune ok"));
    assert!(!stale_dir.exists());
    assert!(dir.join(".garde-fou-cache/v14").is_dir());

    let cl = Cli::try_parse_from(["garde", "cache", "clean"])?;
    let (cl_out, cl_code) = execute_with_policy(&cl, &dir, ColorPolicy::Plain)?;
    assert_eq!(cl_code, ExitCode::SUCCESS);
    assert!(cl_out.contains("cache clean ok"));
    assert!(!dir.join(".garde-fou-cache").exists());
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn uninstall_project_dry_run_and_surgical_eject() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("eject")?;
    init_offline_workspace(&dir)?;
    let agents_path = dir.join("AGENTS.md");
    let agents_raw = fs::read_to_string(&agents_path)?;
    fs::write(
        &agents_path,
        format!("## Notes Équipe\n\nGarder ceci.\n\n{agents_raw}"),
    )?;
    let gi_path = dir.join(".gitignore");
    let gi_raw = fs::read_to_string(&gi_path)?;
    fs::write(&gi_path, format!("*.secret.local\n\n{gi_raw}"))?;
    let custom_skill = dir.join(".agents/skills/mon-skill/SKILL.md");
    fs::create_dir_all(dir.join(".agents/skills/mon-skill"))?;
    fs::write(&custom_skill, "# Custom Skill\n")?;

    let dry = Cli::try_parse_from(["garde", "uninstall", "--project", "--dry-run"])?;
    let (dry_out, dry_code) = execute_with_policy(&dry, &dir, ColorPolicy::Plain)?;
    assert_eq!(dry_code, ExitCode::SUCCESS);
    assert!(dry_out.contains("DRY-RUN"));
    assert!(dir.join("garde-fou.toml").is_file());

    let ej = Cli::try_parse_from(["garde", "eject", "-y"])?;
    let (_, ej_code) = execute_with_policy(&ej, &dir, ColorPolicy::Plain)?;
    assert_eq!(ej_code, ExitCode::SUCCESS);
    assert!(!dir.join("garde-fou.toml").exists());
    assert!(!dir.join("garde.lock").exists());
    assert!(!dir.join(".agents/skills/garde-fou").exists());
    assert!(custom_skill.is_file());
    let agents_after = fs::read_to_string(&agents_path)?;
    assert!(agents_after.contains("## Notes Équipe"));
    assert!(!agents_after.contains("garde-fou managed"));
    let gi_after = fs::read_to_string(&gi_path)?;
    assert!(gi_after.contains("*.secret.local"));
    assert!(!gi_after.contains("garde-fou managed"));
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn watch_snapshot_and_self_uninstall_planner() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_temp_dir("watch")?;
    init_offline_workspace(&dir)?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("src/a.rs"), "pub fn a() {}\n")?;
    let loaded = load_workspace_config(&dir)?;
    let snap1 = capture_snapshot(&dir, &loaded.analyzer, &loaded.cache_dir)?;

    fs::create_dir_all(dir.join(".git"))?;
    fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    fs::create_dir_all(dir.join(".garde-fou-cache/v14"))?;
    fs::write(dir.join(".garde-fou-cache/v14/x.json"), "{}")?;
    let snap2 = capture_snapshot(&dir, &loaded.analyzer, &loaded.cache_dir)?;
    assert_eq!(diff_snapshots(&snap1, &snap2), None);

    fs::write(dir.join("src/a.rs"), "pub fn a() -> u8 { 42 }\n")?;
    let snap3 = capture_snapshot(&dir, &loaded.analyzer, &loaded.cache_dir)?;
    let delta = diff_snapshots(&snap2, &snap3).ok_or("expected watch delta")?;
    assert!(delta.summary_short().contains("src/a.rs"));

    let mise_p = Path::new("/home/u/.local/share/mise/installs/cargo-garde-fou/0.2.1/bin/garde");
    assert_eq!(
        detect_mise_tool_slug(mise_p).as_deref(),
        Some("cargo:garde-fou")
    );
    let self_plan = plan_self_uninstall(mise_p, None);
    assert!(!self_plan.is_empty());
    fs::remove_dir_all(&dir)?;
    Ok(())
}
