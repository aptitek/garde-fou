//! Property-based (`proptest`) suite for TypeScript, React 19 / TSX, JSONC `tsconfig`,
//! and Tier 3 external tool parsers (`oxlint`, `biome`, `tsc`).

use std::path::Path;

use gf_core::{Config, RepoPath};
use gf_parsers::{
    langs::{jsonc::strip_jsonc_to_json, tool_config::extract_tool_config},
    parse_biome_json, parse_oxlint_json, parse_source, parse_tsc_output,
};
use gf_rules::evaluate_document;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn fuzz_utf8_never_panics_ts_tsx_jsonc_and_external_parsers() -> Result<(), String> {
    let mut r = runner();
    let ts_path = RepoPath::new("src/fuzz.ts").map_err(|e| e.to_string())?;
    let tsx_path = RepoPath::new("src/Fuzz.tsx").map_err(|e| e.to_string())?;
    let cfg_path = RepoPath::new("tsconfig.json").map_err(|e| e.to_string())?;
    let root = Path::new("/workspace");

    r.run(&".{0,320}", |arbitrary| {
        let pure_ts =
            parse_source(&ts_path, &arbitrary).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let jsx_tsx =
            parse_source(&tsx_path, &arbitrary).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(pure_ts.effective_lines <= pure_ts.total_lines);
        prop_assert!(jsx_tsx.effective_lines <= jsx_tsx.total_lines);
        let stripped = strip_jsonc_to_json(&arbitrary);
        prop_assert_eq!(
            stripped.chars().filter(|&c| c == '\n').count(),
            arbitrary.chars().filter(|&c| c == '\n').count()
        );
        let cfg_doc = extract_tool_config(&cfg_path, gf_core::Language::Json, &arbitrary)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(cfg_doc.effective_lines <= cfg_doc.total_lines);
        prop_assert!(
            parse_oxlint_json(&arbitrary, root).is_ok()
                || parse_oxlint_json(&arbitrary, root).is_err()
        );
        prop_assert!(
            parse_biome_json(&arbitrary, root).is_ok()
                || parse_biome_json(&arbitrary, root).is_err()
        );
        let tsc_findings =
            parse_tsc_output(&arbitrary, root).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(tsc_findings.len() <= arbitrary.lines().count() + 1);
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn line_number_covariance_holds_under_comment_prefix_shifts_in_tsx() -> Result<(), String> {
    let mut r = runner();
    let path = RepoPath::new("src/Comp.tsx").map_err(|e| e.to_string())?;
    let cfg = Config::strict_defaults();
    let base_tsx = "\
export function Comp({ skip, user }: { skip: boolean; user?: { name: string } }) {
  if (skip) return null;
  const badHook = useMemo(() => 1, []);
  const badChain = user?.name!;
  const badAny: any = badHook;
  return <button>{badChain}{badAny}</button>;
}
";
    let base_doc = parse_source(&path, base_tsx).map_err(|e| e.to_string())?;
    let base_diags = evaluate_document(&base_doc, &cfg);
    assert!(base_diags.len() >= 4);

    r.run(&(1_u32..25_u32), |k| {
        let prefix = "// header\n".repeat(k as usize);
        let shifted_src = format!("{prefix}{base_tsx}");
        let shifted_doc =
            parse_source(&path, &shifted_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let shifted_diags = evaluate_document(&shifted_doc, &cfg);
        prop_assert_eq!(shifted_diags.len(), base_diags.len());
        for (b, s) in base_diags.iter().zip(shifted_diags.iter()) {
            prop_assert_eq!(&b.rule_id, &s.rule_id);
            prop_assert_eq!(s.span.start_line(), b.span.start_line() + k);
            prop_assert_eq!(s.span.end_line(), b.span.end_line() + k);
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn tsx_component_type2_structural_hash_is_invariant_under_identifier_renaming() -> Result<(), String>
{
    let mut r = runner();
    let path_a = RepoPath::new("src/Alpha.tsx").map_err(|e| e.to_string())?;
    let path_b = RepoPath::new("src/Beta.tsx").map_err(|e| e.to_string())?;

    r.run(
        &(
            "[A-Z][a-zA-Z0-9]{2,10}",
            "[A-Z][a-zA-Z0-9]{2,10}",
            "[a-z][a-zA-Z0-9]{2,10}",
            "[a-z][a-zA-Z0-9]{2,10}",
        ),
        |(comp_a, comp_b, var_a, var_b)| {
            let render = |comp: &str, v: &str, lit: &str| {
                format!(
                    "export function {comp}(items: string[]) {{\n\
                       const {v} = items.filter((item) => item !== \"{lit}\");\n\
                       if ({v}.length === 0) {{\n\
                         return null;\n\
                       }}\n\
                       return (\n\
                         <ul className=\"list\">\n\
                           {{{v}.map((entry) => (\n\
                             <li key={{entry}}>{{entry}}</li>\n\
                           ))}}\n\
                         </ul>\n\
                       );\n\
                     }}\n"
                )
            };
            let doc_a = parse_source(&path_a, &render(&comp_a, &var_a, "skip_a"))
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            let doc_b = parse_source(&path_b, &render(&comp_b, &var_b, "other_b"))
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(doc_a.callables[0].structural_hash.is_some());
            prop_assert_eq!(
                doc_a.callables[0].structural_hash,
                doc_b.callables[0].structural_hash
            );
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}
