//! Property-based tests for Type-2 (`BLAKE3`) & Type-3 (`MinHash` 5-gram shingle)
//! AST clone invariance (`GF020`) and `Dockerfile` anti-panic.

use gf_core::{Config, ProjectContext, RepoPath, UastDocument};
use gf_parsers::parse_source;
use gf_rules::evaluate_workspace_sorted;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

#[derive(Clone, Debug)]
struct GeneratedVariant {
    fn_name: String,
    p1: String,
    p2: String,
    local: String,
    lit: u16,
}

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

fn variant_strategy(lo: u16, hi: u16) -> impl Strategy<Value = GeneratedVariant> {
    (
        "fn_[a-z]{3,6}",
        "p1_[a-z]{2,5}",
        "p2_[a-z]{2,5}",
        "acc_[a-z]{2,5}",
        lo..hi,
    )
        .prop_map(|(fn_name, p1, p2, local, lit)| GeneratedVariant {
            fn_name,
            p1,
            p2,
            local,
            lit,
        })
}

fn render_25_line_fn(v: &GeneratedVariant, prefix_stmt: Option<&str>) -> String {
    let GeneratedVariant {
        fn_name,
        p1,
        p2,
        local,
        lit,
    } = v;
    let injected = prefix_stmt.map_or_else(String::new, |stmt| format!("    {stmt}\n"));
    format!(
        "pub fn {fn_name}({p1}: &[i32], {p2}: i32) -> i32 {{\n\
         {injected}\
         \x20   let mut {local} = {lit};\n\
         \x20   for item in {p1} {{\n\
         \x20       if *item > {p2} {{\n\
         \x20           {local} += *item;\n\
         \x20       }} else if *item == 0 {{\n\
         \x20           {local} -= 1;\n\
         \x20       }} else {{\n\
         \x20           {local} += 2;\n\
         \x20       }}\n\
         \x20       if {local} > 100 {{\n\
         \x20           {local} /= 2;\n\
         \x20       }}\n\
         \x20       if {local} < -50 {{\n\
         \x20           {local} = 0;\n\
         \x20       }}\n\
         \x20   }}\n\
         \x20   if {local} < 0 {{\n\
         \x20       return 0;\n\
         \x20   }}\n\
         \x20   if {local} > 1000 {{\n\
         \x20       return 1000;\n\
         \x20   }}\n\
         \x20   {local} + 1\n\
         }}\n"
    )
}

fn parse_variant_doc(
    path: &str,
    v: &GeneratedVariant,
    prefix_stmt: Option<&str>,
) -> Result<UastDocument, TestCaseError> {
    let src = render_25_line_fn(v, prefix_stmt);
    let repo_path = RepoPath::new(path).map_err(|e| TestCaseError::fail(e.to_string()))?;
    parse_source(&repo_path, &src).map_err(|e| TestCaseError::fail(e.to_string()))
}

#[test]
fn type2_clone_hash_is_invariant_under_identifier_and_literal_renaming() -> Result<(), String> {
    let mut tr = runner();
    let strat = (variant_strategy(1, 500), variant_strategy(501, 999));

    tr.run(&strat, |(va, vb)| {
        let doc_a = parse_variant_doc("src/alpha.rs", &va, None)?;
        let doc_b = parse_variant_doc("src/beta.rs", &vb, None)?;
        prop_assert_eq!(
            doc_a.callables[0].structural_hash,
            doc_b.callables[0].structural_hash
        );
        prop_assert_eq!(
            doc_a.callables[0].shingle_sketch,
            doc_b.callables[0].shingle_sketch
        );
        prop_assert!(doc_a.callables[0].ast_node_count >= 40);

        let diags = evaluate_workspace_sorted(
            &[doc_a, doc_b],
            &ProjectContext::default(),
            &Config::strict_defaults(),
        );
        prop_assert!(diags.iter().any(|d| d.rule_id == "GF020"));
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn type3_minhash_shingle_sketch_shares_at_least_7_of_8_registers_with_injected_prefix_stmt()
-> Result<(), String> {
    let mut tr = runner();
    let pad_strat = ("pad_[a-z]{3,6}", 1u16..999u16, prop::bool::ANY);
    let strat = (
        variant_strategy(1, 400),
        variant_strategy(401, 900),
        pad_strat,
    );

    tr.run(&strat, |(va, vb, (pad_var, pad_lit, call_form))| {
        let injected = if call_form {
            format!("{pad_var}({pad_lit});")
        } else {
            format!("let {pad_var} = {pad_lit};")
        };
        let doc_orig = parse_variant_doc("src/orig.rs", &va, None)?;
        let doc_mut = parse_variant_doc("src/mutated.rs", &vb, Some(&injected))?;
        prop_assert_eq!(doc_orig.callables[0].effective_lines, 25);
        let sa = doc_orig.callables[0]
            .shingle_sketch
            .ok_or_else(|| TestCaseError::fail("missing orig shingle_sketch"))?;
        let sb = doc_mut.callables[0]
            .shingle_sketch
            .ok_or_else(|| TestCaseError::fail("missing mutated shingle_sketch"))?;
        let shared = sa.iter().zip(sb.iter()).filter(|(x, y)| x == y).count();
        prop_assert!(
            shared >= 7,
            "expected >= 7/8 matching registers, got {shared}/8"
        );
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn dockerfile_parser_never_panics_on_arbitrary_or_truncated_inputs() -> Result<(), String> {
    let mut tr = runner();
    let strat = prop_oneof![
        ".*",
        "(# escape=`\n)?(FROM [a-z0-9:/@-]*(\n| \\\\\n))?(RUN <<EOF\n[^\n]*(\nEOF)?|ADD .*|ENV .*)?",
    ];

    tr.run(&strat, |raw| {
        let path = RepoPath::new("Dockerfile").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let first = parse_source(&path, &raw).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let second = parse_source(&path, &raw).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(first, second);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
