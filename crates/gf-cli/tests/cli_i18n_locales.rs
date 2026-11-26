//! Bilingual FR/EN CLI flag (`--locale`), embedded `locales/{fr,en}.toml` rendering,
//! and cross-locale `I18N-PLACEHOLDER-MISMATCH` (`GF062`) integration tests.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::Parser;
use gf_cli::{Cli, LocaleChoice, render_localized_check_banner, render_localized_cli_label};
use gf_core::{Config, UiLocale};
use gf_engine::{ScanScope, run_check};

static SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-i18n-cli-{tag}-{}-{n}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn cli_parses_global_locale_flag_and_renders_fr_en_banners()
-> Result<(), Box<dyn std::error::Error>> {
    let fr = Cli::try_parse_from(["garde", "--locale", "fr", "check"])?;
    assert_eq!(fr.locale, LocaleChoice::Fr);
    let en = Cli::try_parse_from(["garde", "check", "--locale", "en"])?;
    assert_eq!(en.locale, LocaleChoice::En);

    let dir = temp_ws("banner")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/lib.rs"),
        "/// Adds one.\npub const fn inc(x: u32) -> u32 { x + 1 }\n",
    )?;
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;

    let fr_banner = render_localized_check_banner(&rep, UiLocale::Fr);
    let en_banner = render_localized_check_banner(&rep, UiLocale::En);
    assert!(fr_banner.contains("Garde-fou :"));
    assert!(en_banner.contains("Garde-fou:"));
    assert!(render_localized_cli_label(UiLocale::Fr, "sync", 3).contains("Synchronisation"));
    assert!(render_localized_cli_label(UiLocale::En, "lock", 0).contains("verified"));
    assert!(render_localized_cli_label(UiLocale::Fr, "commit", 0).contains("conforme"));
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn cross_locale_placeholder_mismatch_triggers_gf062() -> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_ws("mismatch")?;
    fs::create_dir_all(dir.join("locales"))?;
    fs::write(
        dir.join("locales/fr.json"),
        r#"{"welcome": "Bonjour {name}"}"#,
    )?;
    fs::write(
        dir.join("locales/en.json"),
        r#"{"welcome": "Hello {username}"}"#,
    )?;
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    let rep = run_check(&dir, ScanScope::All, &cfg)?;
    assert!(
        rep.diagnostics
            .iter()
            .any(|d| d.rule_id == "GF062" && d.message.contains("I18N-PLACEHOLDER-MISMATCH")),
        "expected I18N-PLACEHOLDER-MISMATCH: {:?}",
        rep.diagnostics
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}
