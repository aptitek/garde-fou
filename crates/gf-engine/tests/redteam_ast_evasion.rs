//! Red Team AST evasion suite verifying local anti-bypass, arity unpacking, boolean-bag,
//! totality, silent-swallow, Law of Demeter, and YAGNI guardrails via `gf_engine::run_check`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, ForbiddenRuleKind, ForbiddenRulesConfig, ScannerPolicy};
use gf_engine::{CheckReport, ScanScope, run_check};

static AST_SEQ: AtomicU64 = AtomicU64::new(0);
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fresh_ws(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = AST_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-rt-ast-{tag}-{nanos}-{seq}"));
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

fn write_ws_file(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

fn eval_file(
    rel: &str,
    src: &str,
    cfg: &Config,
) -> Result<CheckReport, Box<dyn std::error::Error>> {
    let dir = fresh_ws("one")?;
    write_ws_file(&dir, rel, src)?;
    let report = run_check(&dir, ScanScope::All, cfg)?;
    fs::remove_dir_all(&dir)?;
    Ok(report)
}

fn has_rule(report: &CheckReport, rule_id: &str) -> bool {
    report.diagnostics.iter().any(|d| d.rule_id == rule_id)
}

#[test]
fn tuple_packing_and_bool_bags_trigger_gf003_and_gf009_with_clean_controls() -> TestResult {
    let cfg = isolated_config();
    let rs_pack = "pub fn sneak(a: u32, ctx: (&str, u32, u32, bool)) -> u32 { a + ctx.1 }\n";
    let ts_pack = "export function sneakTs(a: number, [b, c, d, e]: [number, number, number, number]): number {\n  return a + b + c + d + e;\n}\n";
    assert!(has_rule(&eval_file("src/lib.rs", rs_pack, &cfg)?, "GF003"));
    assert!(has_rule(
        &eval_file("src/index.ts", ts_pack, &cfg)?,
        "GF003"
    ));

    let rs_ok = "pub struct DomainCtx<'a> { pub tag: &'a str, pub x: u32, pub y: u32, pub on: bool }\npub fn apply(a: u32, ctx: &DomainCtx<'_>) -> u32 { a + ctx.x + ctx.y + u32::from(ctx.on) + u32::from(!ctx.tag.is_empty()) }\n";
    let clean_003 = eval_file("src/lib.rs", rs_ok, &cfg)?;
    assert!(!has_rule(&clean_003, "GF003") && clean_003.is_clean());

    let rs_bag = "pub struct Flags { pub a: bool, pub b: bool, pub c: bool }\n";
    let ts_bag = "export interface UiFlags { open: boolean; modal: boolean; dirty: boolean; }\n";
    assert!(has_rule(&eval_file("src/lib.rs", rs_bag, &cfg)?, "GF009"));
    assert!(has_rule(&eval_file("src/index.ts", ts_bag, &cfg)?, "GF009"));

    let rs_enum = "pub enum Mode { Closed, Open, Dirty }\npub struct CleanFlags { pub mode: Mode, pub pinned: bool }\n";
    let clean_009 = eval_file("src/lib.rs", rs_enum, &cfg)?;
    assert!(!has_rule(&clean_009, "GF009") && clean_009.is_clean());
    Ok(())
}

#[test]
fn all_eight_gf010_constructs_and_granular_rules_isolation_hold() -> TestResult {
    let cfg = isolated_config();
    for snippet in [
        "pub fn f(opt: Option<u32>) -> u32 { opt.unwrap() }\n",
        "pub fn f(res: Result<u32, &'static str>) -> u32 { res.expect(\"boom\") }\n",
        "pub fn f() { panic!(\"boom\"); }\n",
        "pub fn f() -> u32 { todo!() }\n",
        "pub fn f() -> u32 { unimplemented!() }\n",
        "pub fn f(x: u32) -> u32 { dbg!(x) }\n",
        "pub unsafe fn raw() {}\n",
        "pub struct Foo;\nunsafe impl Send for Foo {}\n",
    ] {
        assert!(has_rule(&eval_file("src/lib.rs", snippet, &cfg)?, "GF010"));
    }

    let ok_src = "pub fn inc(res: Result<u32, &'static str>) -> Result<u32, &'static str> { Ok(res? + 1) }\n";
    assert!(!has_rule(&eval_file("src/lib.rs", ok_src, &cfg)?, "GF010"));

    let mut selective = isolated_config();
    let mut rules = ForbiddenRulesConfig::empty();
    rules.set_enabled(ForbiddenRuleKind::Unsafe, true);
    rules.set_enabled(ForbiddenRuleKind::Dbg, false);
    selective.rules = rules;

    let dbg_src = "pub fn f(x: u32) -> u32 { dbg!(x) }\n";
    assert!(!has_rule(
        &eval_file("src/lib.rs", dbg_src, &selective)?,
        "GF010"
    ));
    assert!(has_rule(
        &eval_file("src/lib.rs", "pub unsafe fn raw() {}\n", &selective)?,
        "GF010"
    ));
    Ok(())
}

#[test]
fn silent_error_poisoning_triggers_gf011_across_rust_and_python() -> TestResult {
    let cfg = isolated_config();
    let rs_let = "fn fallible_io() -> Result<u32, &'static str> { Ok(1) }\npub fn swallow() { let _ = fallible_io(); }\n";
    let rs_clos =
        "pub fn swallow(res: Result<u32, &'static str>) -> u32 { res.unwrap_or_else(|_| 0) }\n";
    let py_bad = "def work() -> int:\n    return 1\n\ndef entry() -> int | None:\n    try:\n        return work()\n    except ValueError:\n        return None\n";

    assert!(has_rule(&eval_file("src/lib.rs", rs_let, &cfg)?, "GF011"));
    assert!(has_rule(&eval_file("src/lib.rs", rs_clos, &cfg)?, "GF011"));
    assert!(has_rule(
        &eval_file("src/service.py", py_bad, &cfg)?,
        "GF011"
    ));

    let rs_ok = "fn fallible_io() -> Result<u32, &'static str> { Ok(1) }\npub fn prop() -> Result<u32, &'static str> { fallible_io() }\n";
    let py_ok = "def work() -> int:\n    return 1\n\ndef main() -> int:\n    try:\n        return work()\n    except ValueError as err:\n        raise RuntimeError('failed') from err\n";
    assert!(!has_rule(&eval_file("src/lib.rs", rs_ok, &cfg)?, "GF011"));
    assert!(!has_rule(&eval_file("src/main.py", py_ok, &cfg)?, "GF011"));
    Ok(())
}

#[test]
fn linter_camouflage_triggers_gf070_while_justified_expect_passes() -> TestResult {
    let cfg = isolated_config();
    for (path, code) in [
        (
            "src/lib.rs",
            "#[cfg_attr(all(), allow(clippy::cognitive_complexity))]\npub fn f() {}\n",
        ),
        (
            "src/lib.rs",
            "#![cfg_attr(not( doc ), allow(dead_code))]\npub fn f() {}\n",
        ),
        (
            "src/lib.rs",
            "#[expect(clippy::unwrap_used, reason = \"ok!\")]\npub fn f() {}\n",
        ),
        (
            "src/main.py",
            "value: int = 1  # pyright: ignore[reportGeneralTypeIssues]\n",
        ),
        ("src/main.py", "# mypy: ignore-errors\nvalue: int = 1\n"),
    ] {
        assert!(has_rule(&eval_file(path, code, &cfg)?, "GF070"));
    }

    let justified = "#[expect(clippy::print_stdout, reason = \"dedicated CLI console output boundary\")]\npub fn emit_line() {}\n";
    let clean = eval_file("src/lib.rs", justified, &cfg)?;
    assert!(!has_rule(&clean, "GF070") && clean.is_clean());
    Ok(())
}

#[test]
fn demeter_hidden_bools_commented_code_and_ts_type_contracts_enforce_gf073_to_gf075() -> TestResult
{
    let cfg = isolated_config();
    let dem_bad =
        "pub fn nav(a: &Client) -> Result<u32, &'static str> { Ok(a.get_b()?.c().d.e) }\n";
    let dem_ok = "pub fn pipe(xs: &[u32], f: fn(&u32) -> u32, g: fn(&u32) -> bool) -> Vec<u32> { xs.iter().map(f).filter(g).collect() }\n";
    assert!(has_rule(&eval_file("src/lib.rs", dem_bad, &cfg)?, "GF073"));
    assert!(!has_rule(&eval_file("src/lib.rs", dem_ok, &cfg)?, "GF073"));

    let rs_bools = "pub fn run() -> bool { let f = |a: bool, b: Option<bool>| a && b.unwrap_or(false); f(true, None) }\n";
    let py_bools = "def sync(force=True, prune=False):\n    return force and not prune\n";
    assert!(has_rule(&eval_file("src/lib.rs", rs_bools, &cfg)?, "GF074"));
    assert!(has_rule(
        &eval_file("src/main.py", py_bools, &cfg)?,
        "GF074"
    ));

    let dead_cmt = "pub fn active() -> i32 {\n    // let x = compute();\n    // if x > 0 {\n    //     return x;\n    // }\n    7\n}\n";
    let prose = "/// First line of prose documentation.\n/// Second line explaining domain transitions.\n/// Third line detailing invariant safety.\n/// Fourth line describing output bounds.\n/// Fifth line concluding specification.\npub const fn documented() -> i32 { 7 }\n";
    assert!(has_rule(&eval_file("src/lib.rs", dead_cmt, &cfg)?, "GF074"));
    let doc_clean = eval_file("src/lib.rs", prose, &cfg)?;
    assert!(!has_rule(&doc_clean, "GF074") && doc_clean.is_clean());

    let dir = fresh_ws("gf075")?;
    let port = "export type PaymentGateway = { charge(cents: number): boolean };\n";
    let one_impl = "import type { PaymentGateway } from \"./ports\";\nexport const stripe: PaymentGateway = { charge(cents: number): boolean { return cents > 0; } };\n";
    let two_impls = "import type { PaymentGateway } from \"./ports\";\nexport const stripe: PaymentGateway = { charge(cents: number): boolean { return cents > 0; } };\nexport const mock = { charge(cents: number): boolean { return cents >= 0; } } satisfies PaymentGateway;\n";
    write_ws_file(&dir, "src/ports.ts", port)?;
    write_ws_file(&dir, "src/index.ts", one_impl)?;
    assert!(has_rule(&run_check(&dir, ScanScope::All, &cfg)?, "GF075"));

    write_ws_file(&dir, "src/index.ts", two_impls)?;
    let clean_075 = run_check(&dir, ScanScope::All, &cfg)?;
    fs::remove_dir_all(&dir)?;
    assert!(!has_rule(&clean_075, "GF075") && clean_075.is_clean());
    Ok(())
}
