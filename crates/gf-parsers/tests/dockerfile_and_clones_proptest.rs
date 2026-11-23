//! Property-based tests for Type-2 AST clone rename-invariance and `Dockerfile` anti-panic.

use gf_core::{Config, ProjectContext, RepoPath};
use gf_parsers::parse_source;
use gf_rules::evaluate_workspace_sorted;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

fn render_function_variant(names: (&str, &str, &str, &str), lit: u16) -> String {
    let (fn_name, p1, p2, local) = names;
    format!(
        "pub fn {fn_name}({p1}: &[i32], {p2}: i32) -> i32 {{\n\
         \x20   let mut {local} = {lit};\n\
         \x20   for item in {p1} {{\n\
         \x20       if *item > {p2} {{\n\
         \x20           {local} += *item;\n\
         \x20       }} else if *item == 0 {{\n\
         \x20           {local} -= 1;\n\
         \x20       }}\n\
         \x20       if {local} > 100 {{\n\
         \x20           {local} /= 2;\n\
         \x20       }}\n\
         \x20   }}\n\
         \x20   if {local} < 0 {{\n\
         \x20       return 0;\n\
         \x20   }}\n\
         \x20   {local}\n\
         }}\n"
    )
}

#[test]
fn type2_clone_hash_is_invariant_under_identifier_and_literal_renaming() -> Result<(), String> {
    let mut tr = runner();
    let strat = (
        (
            "fn_[a-z]{3,6}",
            "p1_[a-z]{2,5}",
            "p2_[a-z]{2,5}",
            "acc_[a-z]{2,5}",
            1u16..500u16,
        ),
        (
            "op_[a-z]{3,6}",
            "q1_[a-z]{2,5}",
            "q2_[a-z]{2,5}",
            "sum_[a-z]{2,5}",
            501u16..999u16,
        ),
    );

    tr.run(
        &strat,
        |((f1, a1, b1, loc1, lit1), (f2, a2, b2, loc2, lit2))| {
            let src_a = render_function_variant((&f1, &a1, &b1, &loc1), lit1);
            let src_b = render_function_variant((&f2, &a2, &b2, &loc2), lit2);

            let doc_a = parse_source(
                &RepoPath::new("src/alpha.rs").map_err(|e| TestCaseError::fail(e.to_string()))?,
                &src_a,
            )
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
            let doc_b = parse_source(
                &RepoPath::new("src/beta.rs").map_err(|e| TestCaseError::fail(e.to_string()))?,
                &src_b,
            )
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

            prop_assert_eq!(
                doc_a.callables[0].structural_hash,
                doc_b.callables[0].structural_hash
            );
            prop_assert!(doc_a.callables[0].ast_node_count >= 40);

            let diags = evaluate_workspace_sorted(
                &[doc_a, doc_b],
                &ProjectContext::default(),
                &Config::strict_defaults(),
            );
            prop_assert!(diags.iter().any(|d| d.rule_id == "GF020"));
            Ok(())
        },
    )
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
