//! End-to-end tests for `GF073` anonymous tuple arity (`max_tuple_arity`) and
//! unsafe global/module-scope tuple unpacking (`forbid_global_tuple_unpacking`)
//! across Rust, Python, TypeScript/JavaScript, C#, and Dart.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, ScannerPolicy};
use gf_engine::{CheckReport, ScanScope, run_check};

static SEQ: AtomicU64 = AtomicU64::new(0);
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fresh_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-tuple-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn isolated_config() -> Config {
    let mut cfg = Config::strict_defaults();
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;
    cfg.tools.python.ruff = ScannerPolicy::Off;
    cfg.tools.python.mypy = ScannerPolicy::Off;
    cfg.tools.python.pyright = ScannerPolicy::Off;
    cfg
}

fn eval_one(rel: &str, src: &str, cfg: &Config) -> Result<CheckReport, Box<dyn std::error::Error>> {
    let dir = fresh_ws("one")?;
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, src)?;
    let report = run_check(&dir, ScanScope::All, cfg)?;
    fs::remove_dir_all(&dir)?;
    Ok(report)
}

fn has_gf073_msg(report: &CheckReport, needle: &str) -> bool {
    report
        .diagnostics
        .iter()
        .any(|d| d.rule_id == "GF073" && d.message.contains(needle))
}

#[test]
fn rust_anonymous_tuples_respect_max_tuple_arity_and_exempt_named_tuple_structs() -> TestResult {
    let cfg = isolated_config();
    let bad = "pub fn compute() -> (u32, u32, u32) { (1, 2, 3) }\n";
    let rep_bad = eval_one("src/lib.rs", bad, &cfg)?;
    assert!(has_gf073_msg(&rep_bad, "anonymous tuple"));

    let ok_pair =
        "pub struct Triple(pub u32, pub u32, pub u32);\npub fn pair() -> (u32, u32) { (1, 2) }\n";
    let rep_ok = eval_one("src/lib.rs", ok_pair, &cfg)?;
    assert!(!has_gf073_msg(&rep_ok, "anonymous tuple"));

    let mut strict_zero = isolated_config();
    strict_zero.clean_code.max_tuple_arity = 0;
    let rep_zero = eval_one("src/lib.rs", ok_pair, &strict_zero)?;
    assert!(has_gf073_msg(&rep_zero, "anonymous tuple"));
    Ok(())
}

#[test]
fn python_tuples_and_global_unpacking_enforced_with_except_exempted() -> TestResult {
    let cfg = isolated_config();
    let bad_tuple = "def f() -> tuple[int, str, bool]:\n    return (1, 'a', True)\n";
    assert!(has_gf073_msg(
        &eval_one("src/mod.py", bad_tuple, &cfg)?,
        "anonymous tuple"
    ));

    let bad_global = "a, b = (1, 2)\n";
    assert!(has_gf073_msg(
        &eval_one("src/mod.py", bad_global, &cfg)?,
        "global tuple unpacking"
    ));

    let ok_local = "def f(items: list[tuple[int, int]]) -> int:\n    try:\n        a, b = items[0]\n        return a + b\n    except (ValueError, TypeError, KeyError):\n        return 0\n";
    let rep_ok = eval_one("src/mod.py", ok_local, &cfg)?;
    assert!(!has_gf073_msg(&rep_ok, "anonymous tuple"));
    assert!(!has_gf073_msg(&rep_ok, "global tuple unpacking"));

    let mut allow_global = isolated_config();
    allow_global.clean_code.forbid_global_tuple_unpacking = false;
    assert!(!has_gf073_msg(
        &eval_one("src/mod.py", bad_global, &allow_global)?,
        "global tuple unpacking"
    ));
    Ok(())
}

#[test]
fn typescript_tuples_and_global_unpacking_are_detected() -> TestResult {
    let cfg = isolated_config();
    let bad_ts = "export type Tri = [number, string, boolean];\nexport const [x, y] = [1, 2];\n";
    let rep = eval_one("src/app.ts", bad_ts, &cfg)?;
    assert!(has_gf073_msg(&rep, "anonymous tuple"));
    assert!(has_gf073_msg(&rep, "global tuple unpacking"));

    let ok_ts = "export function run(): number {\n  const [a, b]: [number, number] = [1, 2];\n  return a + b;\n}\n";
    let rep_ok = eval_one("src/app.ts", ok_ts, &cfg)?;
    assert!(!has_gf073_msg(&rep_ok, "anonymous tuple"));
    assert!(!has_gf073_msg(&rep_ok, "global tuple unpacking"));
    Ok(())
}

#[test]
fn csharp_tuples_and_top_level_unpacking_are_detected() -> TestResult {
    let cfg = isolated_config();
    let bad_cs = "var (a, b) = (1, 2);\n(int, string, bool) tri = (1, \"x\", true);\n";
    let rep = eval_one("src/Program.cs", bad_cs, &cfg)?;
    assert!(
        has_gf073_msg(&rep, "anonymous tuple"),
        "diags: {:?}",
        rep.diagnostics
    );
    assert!(
        has_gf073_msg(&rep, "global tuple unpacking"),
        "diags: {:?}",
        rep.diagnostics
    );

    let ok_cs = "public static class MathUtil {\n  public static (int, int) Pair() {\n    var (a, b) = (1, 2);\n    return (a, b);\n  }\n}\n";
    let rep_ok = eval_one("src/MathUtil.cs", ok_cs, &cfg)?;
    assert!(!has_gf073_msg(&rep_ok, "anonymous tuple"));
    assert!(!has_gf073_msg(&rep_ok, "global tuple unpacking"));
    Ok(())
}

#[test]
fn dart_records_and_top_level_record_unpacking_are_detected() -> TestResult {
    let cfg = isolated_config();
    let bad_dart =
        "final (a, b) = (1, 2);\n(int, String, bool) build() {\n  return (1, 'ok', true);\n}\n";
    let rep = eval_one("lib/main.dart", bad_dart, &cfg)?;
    assert!(has_gf073_msg(&rep, "anonymous tuple"));
    assert!(has_gf073_msg(&rep, "global tuple unpacking"));

    let ok_dart = "(int, int) pair() {\n  final (a, b) = (1, 2);\n  return (a, b);\n}\n";
    let rep_ok = eval_one("lib/main.dart", ok_dart, &cfg)?;
    assert!(!has_gf073_msg(&rep_ok, "anonymous tuple"));
    assert!(!has_gf073_msg(&rep_ok, "global tuple unpacking"));
    Ok(())
}

fn eval_ws(
    files: &[(&str, &str)],
    cfg: &Config,
) -> Result<CheckReport, Box<dyn std::error::Error>> {
    let dir = fresh_ws("arch")?;
    for (rel, content) in files {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
    }
    let rep = run_check(&dir, ScanScope::All, cfg)?;
    fs::remove_dir_all(&dir)?;
    Ok(rep)
}

#[test]
fn architecture_purity_enforces_gf078_on_domain_crate() -> TestResult {
    let mut arch = isolated_config();
    arch.architecture
        .no_std_crates
        .insert(String::from("domain"));
    arch.architecture
        .io_allowed_crates
        .insert(String::from("shell"));
    arch.architecture
        .console_allowed_crates
        .insert(String::from("shell"));
    let sh = (
        "crates/shell/src/lib.rs",
        "use std::fs;\npub fn f() { if fs::metadata(\".\").is_ok() { println!(\"ok\"); } }\n",
    );
    let dom = "crates/domain/src/lib.rs";
    for leak in [
        "pub const fn a() -> u32 { 1 }\n",
        "#![no_std]\nuse std::fs;\n",
        "#![no_std]\npub fn f() { println!(\"leak\"); }\n",
    ] {
        assert!(
            eval_ws(&[sh, (dom, leak)], &arch)?
                .diagnostics
                .iter()
                .any(|d| d.rule_id == "GF078")
        );
    }
    assert!(
        !eval_ws(
            &[sh, (dom, "#![no_std]\npub const fn a() -> u32 { 1 }\n")],
            &arch
        )?
        .diagnostics
        .iter()
        .any(|d| d.rule_id == "GF078")
    );
    Ok(())
}
