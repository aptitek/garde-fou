//! Red Team workspace-graph, metamorphic Type-3 clone (`proptest`), dead-export homonymy,
//! TDD theatre, `Scripts` spoofing, `LCOV`/`CRAP`, and `[architecture]` evasion suite.

use std::fmt::Write as _;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, CoverageConfig, FeatureId, RepoPath, ScannerPolicy, TddParityGate};
use gf_engine::{CheckReport, ScanScope, run_check};
use gf_parsers::parse_source;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

static GRAPH_SEQ: AtomicU64 = AtomicU64::new(0);
type Res<T> = Result<T, Box<dyn std::error::Error>>;
type TestResult = Res<()>;

fn base_cfg() -> Config {
    let mut cfg = Config::strict_defaults();
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;
    cfg.tools.python.ruff = ScannerPolicy::Off;
    cfg.tools.python.mypy = ScannerPolicy::Off;
    cfg.tools.python.pyright = ScannerPolicy::Off;
    cfg
}

fn eval_ws(files: &[(&str, &str)], cfg: &Config) -> Res<CheckReport> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = GRAPH_SEQ.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("gf-rt-grp-{nanos}-{seq}"));
    for (rel, content) in files {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
    }
    let rep = run_check(&root, ScanScope::All, cfg)?;
    fs::remove_dir_all(&root)?;
    Ok(rep)
}

fn ws_has(files: &[(&str, &str)], cfg: &Config, rule: &str) -> Res<bool> {
    let rep = eval_ws(files, cfg)?;
    Ok(rep.diagnostics.iter().any(|d| d.rule_id == rule))
}

fn render_clone_fn(ids: [&str; 4], lit: u32, head: &str, tail: &str) -> String {
    let [fn_id, xs, lim, acc] = ids;
    format!(
        "pub fn {fn_id}({xs}: &[i32], {lim}: i32) -> i32 {{\n{head}    let mut {acc} = {lit}_i32;\n\
         \x20   for item in {xs} {{\n        if *item > {lim} {{ {acc} = {acc}.saturating_add(*item * 2 + 1); }} else {{ {acc} -= 1; }}\n\
         \x20       if {acc} > 500 {{ {acc} = {acc} / 2 + 3; }}\n    }}\n\
         \x20   let s1 = {acc}.wrapping_add({lim}).wrapping_mul(3);\n    let s2 = s1.wrapping_sub({lim}).wrapping_add(7);\n\
         \x20   let s3 = s2 ^ ({acc} & 255);\n    let s4 = s3.saturating_add(s1 / 2);\n\
         \x20   let s5 = s4.wrapping_add(s2 % 16);\n    let s6 = s5.saturating_sub(3);\n\
         \x20   let s7 = s6.wrapping_mul(2);\n    let s8 = s7.saturating_add(5);\n\
         \x20   let s9 = s8 ^ 15;\n    let s10 = s9.saturating_add(1);\n\
         \x20   if s10 < 0 {{\n        return 0;\n    }}\n    if s10 > 10_000 {{\n        return 10_000;\n    }}\n{tail}    s10\n}}\n"
    )
}

#[test]
fn proptest_type3_near_miss_clones_always_trigger_gf020_despite_noise() -> TestResult {
    let mut tr = TestRunner::new(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let cfg = base_cfg();
    let ids = ("fn_[a-z]{3}", "x_[a-z]{2}", "l_[a-z]{2}", "a_[a-z]{2}");
    let nums = (1u32..400, 401u32..900, 1usize..=2, prop::bool::ANY);
    tr.run(&(ids, ids, nums), |(va, vb, (la, lb, noise_n, at_tail))| {
        let pad = "    let noise_a = 42_u32;\n".repeat(noise_n);
        let a_ids = [va.0.as_str(), va.1.as_str(), va.2.as_str(), va.3.as_str()];
        let a = render_clone_fn(a_ids, la, "", "");
        let b_fn = format!("b_{}", vb.0);
        let b_ids = [b_fn.as_str(), vb.1.as_str(), vb.2.as_str(), vb.3.as_str()];
        let (h, t) = if at_tail {
            ("", pad.as_str())
        } else {
            (pad.as_str(), "")
        };
        let b = render_clone_fn(b_ids, lb, h, t);

        let pa = RepoPath::new("src/alpha.rs").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let pb = RepoPath::new("src/beta.rs").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let da = parse_source(&pa, &a).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let db = parse_source(&pb, &b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(da.callables[0].effective_lines >= 22 && da.callables[0].ast_node_count >= 50);
        let (ha, hb) = (
            da.callables[0].structural_hash,
            db.callables[0].structural_hash,
        );
        prop_assert_ne!(ha, hb);

        let ws = [("src/alpha.rs", a.as_str()), ("src/beta.rs", b.as_str())];
        let hit = ws_has(&ws, &cfg, "GF020").map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(hit);
        Ok(())
    })
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let a_src = render_clone_fn(["ca", "xs", "l", "a"], 5, "", "");
    let diff = "pub const fn tag(c: u8) -> &'static str { match c { 0 => \"a\", _ => \"b\" } }\n";
    let ok_ws = [("src/alpha.rs", a_src.as_str()), ("src/b.rs", diff)];
    assert!(!ws_has(&ok_ws, &cfg, "GF020")?);
    Ok(())
}

#[test]
fn homonymy_tarjan_inline_paths_and_atomic_reexports_enforce_gf021_gf071_gf072() -> TestResult {
    let cfg = base_cfg();
    let orph_s = "pub fn parse() -> u32 { 1 }\npub(crate) fn helper_secret() -> u32 { 2 }\n";
    let hom_s = "mod orphan;\npub struct S { pub parse: u32 }\npub fn f() -> u32 { \"42\".parse::<u32>().unwrap_or(0) }\n";
    let live_s = "mod orphan;\npub fn f() -> u32 { orphan::parse() + orphan::helper_secret() }\n";
    let bad_21 = [("src/orphan.rs", orph_s), ("src/lib.rs", hom_s)];
    let ok_21 = [("src/orphan.rs", orph_s), ("src/lib.rs", live_s)];
    let rep_21 = eval_ws(&bad_21, &cfg)?.diagnostics;
    assert_eq!(rep_21.iter().filter(|d| d.rule_id == "GF021").count(), 2);
    assert!(!ws_has(&ok_21, &cfg, "GF021")?);

    let d_fn = "pub fn ping() { crate::a::b::e::pong(); }\n";
    let e_fn = "pub fn pong() { crate::a::b::c::d::ping(); }\n";
    let e_ok = "pub const fn pong() -> u32 { 1 }\n";
    let mut tree = [
        ("src/lib.rs", "pub mod a;\n"),
        ("src/a/mod.rs", "pub mod b;\n"),
        ("src/a/b/mod.rs", "pub mod c;\npub mod e;\n"),
        ("src/a/b/c/mod.rs", "pub mod d;\n"),
        ("src/a/b/c/d.rs", d_fn),
        ("src/a/b/e.rs", e_fn),
    ];
    assert!(ws_has(&tree, &cfg, "GF071")?);
    tree[5] = ("src/a/b/e.rs", e_ok);
    assert!(!ws_has(&tree, &cfg, "GF071")?);

    let mut ui = base_cfg();
    ui.project.features.insert(FeatureId::Ui);
    let btn = "src/components/atoms/Button.tsx";
    let ico = "src/components/atoms/Icon.tsx";
    let reexp = "export { Header } from \"../organisms/Header\";\n";
    let dyn_s = "export async function f() { return await import(\"../services/auth\"); }\n";
    let ico_s = "export function Icon() { return <span />; }\n";
    let btn_s = "import { Icon } from \"./Icon\";\nexport function Button() { return <Icon />; }\n";
    assert!(ws_has(&[(btn, reexp)], &ui, "GF072")?);
    assert!(ws_has(&[(btn, dyn_s)], &ui, "GF072")?);
    assert!(!ws_has(&[(ico, ico_s), (btn, btn_s)], &ui, "GF072")?);
    Ok(())
}

#[test]
fn tdd_theatre_and_fake_script_coverage_trigger_gf076_and_gf065() -> TestResult {
    let mut tdd = base_cfg();
    tdd.tdd.parity_gates.insert(TddParityGate::CompanionTest);
    let gs = "export function runGhost(): number { return 1; }\n";
    let gb = "export function helper(): boolean { return true; }\n";
    let go = "import { runGhost } from \"../src/ghost\";\ntest(\"ok\", () => { expect(runGhost()).toEqual(1); });\n";
    let g_bad = [("src/ghost.ts", gs), ("tests/ghost.test.ts", gb)];
    let g_ok = [("src/ghost.ts", gs), ("tests/ghost.test.ts", go)];
    let g_rep = eval_ws(&g_bad, &tdd)?;
    let gf76: Vec<&str> = g_rep
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF076")
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(gf76, vec!["src/ghost.ts", "tests/ghost.test.ts"]);
    assert!(!ws_has(&g_ok, &tdd, "GF076")?);

    let cfg = base_cfg();
    let s1 = "fn check_something() {}\nfn verify_other() {}\n#[test]\nfn t() { check_something(); verify_other(); }\n";
    let s2 = "#[test]\nfn t() { let x = 3; assert_eq!(x + 1, x + 1); }\n";
    let s3 = "test(\"t\", () => { const val = 5; expect(val).toEqual(val); });\n";
    let s4 = "def test_t() -> None:\n    total = 5\n    assert total == total\n";
    let s5 = "fn compute() -> Result<u32, &'static str> { Ok(9) }\n#[test]\nfn weak() { assert!(compute().is_ok()); }\n";
    let c_a = [("tests/h.rs", s1), ("tests/r.rs", s2)];
    let c_b = [("tests/r.test.ts", s3), ("tests/test_r.py", s4)];
    for (p, s) in c_a.into_iter().chain(c_b).chain([("tests/w.rs", s5)]) {
        assert!(ws_has(&[(p, s)], &cfg, "GF076")?);
    }

    let sh = ("scripts/deploy.sh", "#!/bin/sh\necho ok\n");
    let fk_s = "#[test]\nfn t() { let s = \"scripts/deploy.sh\"; assert_eq!(s.len(), 17); }\n";
    let rl_s = "#[test]\nfn t() { let c = std::process::Command::new(\"sh\").arg(\"scripts/deploy.sh\").status(); assert!(c.is_ok() || c.is_err()); assert_eq!(2, 2); }\n";
    assert!(ws_has(&[sh, ("tests/f.rs", fk_s)], &cfg, "GF065")?);
    assert!(!ws_has(&[sh, ("tests/f.rs", rl_s)], &cfg, "GF065")?);
    Ok(())
}

#[test]
fn crap_and_branch_gates_enforce_gf050_and_gf051() -> TestResult {
    let mut cov = base_cfg();
    cov.coverage = CoverageConfig {
        min_line_coverage: Some(85),
        min_branch_coverage: Some(80),
        max_crap_score: Some(30),
        lcov_path: Some(String::from("lcov.info")),
    };
    let lib_s = "pub mod risky;\npub const fn ok_val(x: u32) -> u32 { risky::route(x) + 1 }\n";
    let rsk_s = "pub const fn route(x: u32) -> u32 {\n    match x {\n        1 => 10, 2 => 20,\n        3 => 30, 4 => 40,\n        5 => 50, 6 => 60,\n        7 => 70,\n        8 => 80,\n        _ => 90,\n    }\n}\n";
    let (lib, rsk) = (("src/lib.rs", lib_s), ("src/risky.rs", rsk_s));
    let mut lbad = String::from("TN:\nSF:src/lib.rs\n");
    for i in 0..70 {
        writeln!(lbad, "DA:{},1", i + 1)?;
        if i < 10 {
            writeln!(lbad, "BRDA:2,0,{i},{}", usize::from(i < 4))?;
        }
    }
    lbad.push_str("end_of_record\nSF:src/risky.rs\n");
    let lok_hdr = "TN:\nSF:src/lib.rs\nDA:1,1\nBRDA:2,0,0,1\nend_of_record\nSF:src/risky.rs\n";
    let mut lok = String::from(lok_hdr);
    for l in 1..=10 {
        writeln!(lbad, "DA:{l},{}", usize::from(l <= 2))?;
        writeln!(lok, "DA:{l},1")?;
    }
    lbad.push_str("end_of_record\n");
    lok.push_str("end_of_record\n");
    let bad = eval_ws(&[lib, rsk, ("lcov.info", &lbad)], &cov)?.diagnostics;
    assert!(bad.iter().any(|d| d.rule_id == "GF050") && bad.iter().any(|d| d.rule_id == "GF051"));
    let ok = eval_ws(&[lib, rsk, ("lcov.info", &lok)], &cov)?;
    assert!(ok.is_clean());
    Ok(())
}

#[test]
fn architecture_purity_enforces_gf078_on_domain_crate() -> TestResult {
    let mut arch = base_cfg();
    let a_cfg = &mut arch.architecture;
    a_cfg.no_std_crates.insert(String::from("domain"));
    a_cfg.io_allowed_crates.insert(String::from("shell"));
    a_cfg.console_allowed_crates.insert(String::from("shell"));
    let sh_s =
        "use std::fs;\npub fn f() { if fs::metadata(\".\").is_ok() { println!(\"ok\"); } }\n";
    let sh = ("crates/shell/src/lib.rs", sh_s);
    let dom = "crates/domain/src/lib.rs";
    for leak in [
        "pub const fn a() -> u32 { 1 }\n",
        "#![no_std]\nuse std::fs;\n",
        "#![no_std]\npub fn f() { println!(\"leak\"); }\n",
    ] {
        assert!(ws_has(&[sh, (dom, leak)], &arch, "GF078")?);
    }
    let ok_dom = "#![no_std]\npub const fn a() -> u32 { 1 }\n";
    assert!(!ws_has(&[sh, (dom, ok_dom)], &arch, "GF078")?);
    Ok(())
}
