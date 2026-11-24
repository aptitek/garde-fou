//! Integration and property-based tests for Lot A AST guardrails (`GF003`, `GF009`,
//! `GF010`, `GF011`, `GF021`, `GF070`–`GF075`, `GF078`).

use gf_core::{
    ContractImpl, EffectSightingKind, ForbiddenRuleKind, KissYagniKind, RepoPath, UastDocument,
    UastKind,
};
use gf_parsers::parse_source;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn collect_forbidden(doc: &UastDocument) -> Vec<(ForbiddenRuleKind, &str)> {
    doc.root
        .children
        .iter()
        .filter_map(|n| match &n.kind {
            UastKind::ForbiddenCall { rule, symbol } => Some((*rule, symbol.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn anti_cheat_catches_nested_cfg_attr_warn_and_weak_expect()
-> Result<(), Box<dyn std::error::Error>> {
    let bad_rs = "\
#[cfg_attr(all(), allow(clippy::unwrap_used))]
#[warn(dead_code)]
#[expect(dead_code, reason = \"ok\")]
pub fn bypass() {}
";
    let doc = parse_source(&RepoPath::new("src/bypass.rs")?, bad_rs)?;
    assert_eq!(doc.bypass_findings.len(), 3);

    let good_rs = "\
/// Legitimate doc referencing clippy::unwrap_used without bypassing anything.
#[expect(dead_code, reason = \"architectural FFI boundary wrapper\")]
pub fn legitimate() -> &'static str {
    \"#[cfg_attr(all(), allow(dead_code))]\"
}
";
    let clean = parse_source(&RepoPath::new("src/clean.rs")?, good_rs)?;
    assert_eq!(clean.bypass_findings, Vec::new());

    let py_bypasses = "# pyright: ignore[reportGeneralTypeIssues]\n# flake8: noqa\nx = 1\n";
    let py_doc = parse_source(&RepoPath::new("src/mod.py")?, py_bypasses)?;
    assert_eq!(py_doc.bypass_findings.len(), 2);
    Ok(())
}

#[test]
fn anti_cheat_detects_commented_out_code_without_prose_false_positives()
-> Result<(), Box<dyn std::error::Error>> {
    let src = "\
// Normal explanatory prose describing our domain invariants
// across three consecutive lines without dead code markers
// so developers can document algorithms clearly.
pub fn active() -> i32 {
    // let stale = compute_old();
    // if stale > 0 {
    //     return stale;
    // }
    42
}
";
    let doc = parse_source(&RepoPath::new("src/debt.rs")?, src)?;
    let commented: Vec<_> = doc
        .kiss_yagni_findings
        .iter()
        .filter(|f| f.kind == KissYagniKind::CommentedOutCode)
        .collect();
    assert_eq!(commented.len(), 1);
    assert_eq!(commented[0].measured, 4);
    Ok(())
}

#[test]
fn rust_guardrails_detect_gf010_gf011_and_cfg_test_scope() -> Result<(), Box<dyn std::error::Error>>
{
    let src = "\
pub unsafe fn raw_op(opt: Option<i32>) -> i32 {
    let _ = crate::secret::audit();
    let v = opt.unwrap();
    let w = opt.expect(\"present\");
    let z = opt.unwrap_or_else(|_| 0);
    if v < 0 { todo!(); }
    v + w + z
}

#[cfg(test)]
mod tests {
    #[test]
    fn unit() {
        let ok = Some(1).unwrap();
        if ok == 0 { panic!(\"zero\"); }
        unsafe { core::hint::spin_loop(); }
        dbg!(ok);
    }
}
";
    let doc = parse_source(&RepoPath::new("crates/core/src/ops.rs")?, src)?;
    let rules: Vec<ForbiddenRuleKind> = collect_forbidden(&doc)
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    assert!(rules.contains(&ForbiddenRuleKind::Unwrap));
    assert!(rules.contains(&ForbiddenRuleKind::Expect));
    assert!(rules.contains(&ForbiddenRuleKind::Todo));
    assert!(rules.contains(&ForbiddenRuleKind::Dbg));
    assert!(!rules.contains(&ForbiddenRuleKind::Panic));
    assert_eq!(
        rules
            .iter()
            .filter(|&&r| r == ForbiddenRuleKind::Unsafe)
            .count(),
        2
    );
    assert_eq!(
        rules
            .iter()
            .filter(|&&r| r == ForbiddenRuleKind::SilentErrorSwallow)
            .count(),
        2
    );
    Ok(())
}

#[test]
fn rust_effects_bool_fields_visibility_and_inline_imports() -> Result<(), Box<dyn std::error::Error>>
{
    let src = "\
#![no_std]
use std::fs;

pub struct ConfigFlags { pub a: bool, pub b: bool, pub c: bool, pub limit: u32 }
pub(crate) fn run() {
    crate::secret::audit();
    println!(\"trace\");
}
";
    let doc = parse_source(&RepoPath::new("crates/core/src/lib.rs")?, src)?;
    assert_eq!(doc.types[0].bool_field_count, 3);
    assert_eq!(doc.exports.len(), 2);
    assert!(!doc.exports[0].is_crate_internal);
    assert!(doc.exports[1].is_crate_internal);
    assert!(
        doc.imports
            .iter()
            .any(|imp| imp.module_source == "crate::secret::audit")
    );
    let effects: Vec<EffectSightingKind> = doc.effect_sightings.iter().map(|e| e.kind).collect();
    assert!(effects.contains(&EffectSightingKind::HasNoStdAttr));
    assert!(effects.contains(&EffectSightingKind::StdUsage));
    assert!(effects.contains(&EffectSightingKind::IoImportOrCall));
    assert!(effects.contains(&EffectSightingKind::ConsoleOutput));
    Ok(())
}

#[test]
fn python_bool_fields_tuple_arity_and_silent_fallback_except()
-> Result<(), Box<dyn std::error::Error>> {
    let py = "\
class FeatureBag:
    enabled: bool = True
    verbose: bool = False
    strict: bool = True
    name: str = 'x'

    def run(self, coords: tuple[int, str, float]) -> int:
        try:
            return int(coords[1])
        except ValueError:
            return 0
";
    let doc = parse_source(&RepoPath::new("src/bag.py")?, py)?;
    assert_eq!(doc.types[0].bool_field_count, 3);
    assert_eq!(doc.callables[0].param_count, 3);
    let rules = collect_forbidden(&doc);
    assert_eq!(
        rules,
        vec![(
            ForbiddenRuleKind::SilentErrorSwallow,
            "except: return fallback"
        )]
    );
    Ok(())
}

#[test]
fn kiss_demeter_bool_params_and_ts_structural_contracts() -> Result<(), Box<dyn std::error::Error>>
{
    let rs = "\
pub fn train(a: Client, items: &[u32], flag: Option<bool>) -> Result<u32, Err> {
    let clean: Vec<u32> = items.iter().copied().collect();
    let leaf = a.b()?.get_c().d.e;
    Ok(leaf + clean[0] + u32::from(flag.unwrap_or(false)))
}
";
    let rs_doc = parse_source(&RepoPath::new("src/demeter.rs")?, rs)?;
    let chains: Vec<u32> = rs_doc
        .kiss_yagni_findings
        .iter()
        .filter(|f| f.kind == KissYagniKind::PropertyChain)
        .map(|f| f.measured)
        .collect();
    assert_eq!(chains, vec![5]);
    assert!(
        rs_doc
            .kiss_yagni_findings
            .iter()
            .any(|f| f.kind == KissYagniKind::ExcessiveBoolParams && f.measured == 1)
    );

    let ts = "\
export type UserRepo = { find(id: string): string };
const memRepo = { find(id: string) { return id; } } satisfies UserRepo;
";
    let ts_doc = parse_source(&RepoPath::new("src/repo.ts")?, ts)?;
    assert_eq!(ts_doc.contracts.len(), 1);
    assert_eq!(ts_doc.contracts[0].name, "UserRepo");
    assert!(ts_doc.contract_impls.contains(&ContractImpl {
        contract: String::from("UserRepo"),
        implementor: String::from("memRepo"),
    }));
    Ok(())
}

#[test]
fn proptest_rust_tuple_packing_unpacks_exact_deployed_arity() -> Result<(), String> {
    let mut tr = TestRunner::new(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    tr.run(&(2usize..7usize), |tuple_len| {
        let elems = vec!["u32"; tuple_len].join(", ");
        let src = format!("pub fn packed(head: u32, bag: ({elems})) -> u32 {{ head }}\n");
        let path = RepoPath::new("src/arity.rs").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = parse_source(&path, &src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let expected = u32::try_from(1 + tuple_len).unwrap_or(0);
        prop_assert_eq!(doc.callables.len(), 1);
        prop_assert_eq!(doc.callables[0].param_count, expected);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
