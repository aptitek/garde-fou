//! Property-based anti-panic and structural invariant tests for all `Tree-sitter` extractors.

use gf_core::RepoPath;
use gf_parsers::parse_source;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn arbitrary_utf8_never_panics_across_all_grammars() -> Result<(), String> {
    let mut tr = runner();
    let paths = [
        "fuzz.rs",
        "fuzz.ts",
        "fuzz.tsx",
        "fuzz.js",
        "fuzz.py",
        "fuzz.toml",
        "fuzz.css",
        "fuzz.md",
        "fuzz.svg",
        "fuzz.json",
        "fuzz.sh",
        "Dockerfile",
    ];
    let strat = (0..paths.len(), ".*");

    tr.run(&strat, |(idx, arbitrary_src)| {
        let repo_path =
            RepoPath::new(paths[idx]).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let first = parse_source(&repo_path, &arbitrary_src)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        let second = parse_source(&repo_path, &arbitrary_src)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        prop_assert_eq!(&first, &second);
        prop_assert!(first.effective_lines <= first.total_lines);
        for callable in &first.callables {
            prop_assert!(callable.cyclomatic_complexity >= 1);
            prop_assert!(callable.span.start_line() >= 1);
            prop_assert!(callable.span.end_line() >= callable.span.start_line());
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn structured_nested_ifs_monotonically_increase_complexity() -> Result<(), String> {
    use core::fmt::Write as _;

    let mut tr = runner();
    tr.run(&(1usize..6usize), |depth| {
        let mut rust_src = String::from("pub fn f(x: i32) -> i32 {\n");
        for level in 0..depth {
            let _ = writeln!(rust_src, "if x > {level} {{");
        }
        rust_src.push_str("return x;\n");
        for _ in 0..depth {
            rust_src.push_str("}\n");
        }
        rust_src.push_str("0\n}\n");

        let path = RepoPath::new("nested.rs").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = parse_source(&path, &rust_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(doc.syntax_errors, Vec::new());
        prop_assert_eq!(doc.callables.len(), 1);

        let d_u32 = u32::try_from(depth).unwrap_or(1);
        let expected_cognitive = d_u32 * (d_u32 + 1) / 2;
        prop_assert_eq!(doc.callables[0].max_nesting_depth, d_u32);
        prop_assert_eq!(doc.callables[0].cyclomatic_complexity, d_u32 + 1);
        prop_assert_eq!(doc.callables[0].cognitive_complexity, expected_cognitive);
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn multibyte_utf8_comments_never_panic_anti_cheat() -> Result<(), Box<dyn std::error::Error>> {
    for sample in ["//\"ࠀ", "//\"𪌴", "/* ࠀ:x */", "# \"ࠀ"] {
        for ext in ["fuzz.rs", "fuzz.ts", "fuzz.py", "fuzz.sh", "Dockerfile"] {
            let path = RepoPath::new(ext)?;
            let doc = parse_source(&path, sample)?;
            assert_eq!(doc.bypass_findings, Vec::new());
            assert_eq!(doc.kiss_yagni_findings, Vec::new());
        }
    }
    Ok(())
}
