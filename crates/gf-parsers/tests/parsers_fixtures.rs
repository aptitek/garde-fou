//! Realistic multi-language fixtures testing clean vs dirty code, TS module graph,
//! homonymy-safe references (`GF021`), TDD inspection (`GF076`), and script coverage (`GF065`).

use gf_core::{Config, EffectSightingKind, Language, RepoPath};
use gf_parsers::{ProjectMarker, detect_language, detect_project_marker, parse_source};
use gf_rules::evaluate_document;

#[test]
fn rust_clean_fixture_passes_all_rules() -> Result<(), Box<dyn core::error::Error>> {
    let clean_src = r"use core::fmt;

pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn norm_l1(&self, scale: i32) -> i32 {
        if scale > 0 {
            (self.x + self.y) * scale
        } else {
            0
        }
    }
}
";
    let doc = parse_source(&RepoPath::new("src/point.rs")?, clean_src)?;
    assert_eq!(doc.language, Language::Rust);
    assert_eq!(doc.imports.len(), 1);
    assert_eq!(doc.exports.len(), 1);
    assert_eq!(doc.types.len(), 2);
    assert_eq!(doc.callables.len(), 1);
    assert_eq!(doc.callables[0].param_count, 1);
    assert_eq!(doc.callables[0].cyclomatic_complexity, 2);
    assert_eq!(doc.callables[0].cognitive_complexity, 1);
    assert_eq!(
        evaluate_document(&doc, &Config::strict_defaults()),
        Vec::new()
    );
    Ok(())
}

#[test]
fn rust_spaghetti_and_syntax_errors_are_flagged() -> Result<(), Box<dyn core::error::Error>> {
    let dirty_src = r"pub fn spaghetti(a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 {
    if a > 0 && b > 0 {
        for i in 0..a {
            if i % 2 == 0 || c > 0 {
                while b > d {
                    if e > 0 {
                        return i;
                    }
                }
            }
        }
    }
    0
}
";
    let dirty_doc = parse_source(&RepoPath::new("src/spaghetti.rs")?, dirty_src)?;
    let f = &dirty_doc.callables[0];
    assert_eq!(f.param_count, 5);
    assert!(f.cognitive_complexity > 10);
    assert_eq!(f.max_nesting_depth, 5);
    let rule_ids: Vec<String> = evaluate_document(&dirty_doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| d.rule_id)
        .collect();
    for expected in ["GF003", "GF004", "GF006"] {
        assert!(rule_ids.iter().any(|r| r == expected));
    }

    let broken_doc = parse_source(
        &RepoPath::new("src/broken.rs")?,
        "pub fn oops( { let x = ; }",
    )?;
    assert_ne!(broken_doc.syntax_errors, Vec::new());
    Ok(())
}

#[test]
fn typescript_imports_reexports_dynamic_calls_bools_and_console()
-> Result<(), Box<dyn core::error::Error>> {
    let ts_src = r#"import { alpha, beta as b } from "./alpha";
export { Button } from "../molecules/Card";

export interface Flags {
    a: boolean;
    b: boolean;
    c: boolean;
}

export const compute = async (x: number, pair: [string, boolean]): Promise<number> => {
    console.warn("trace");
    const mod = await import("./lazy");
    const cjs = require("./legacy");
    return x + pair.length + (mod ? 1 : 0) + (cjs ? 0 : 1);
};
"#;
    let doc = parse_source(&RepoPath::new("src/app.ts")?, ts_src)?;
    assert_eq!(doc.imports.len(), 4);
    assert_eq!(doc.imports[0].symbols, vec!["alpha", "beta"]);
    assert_eq!(doc.imports[1].module_source, "../molecules/Card");
    assert_eq!(doc.imports[2].module_source, "./lazy");
    assert_eq!(doc.imports[3].module_source, "./legacy");
    assert_eq!(doc.types[0].bool_field_count, 3);
    assert_eq!(doc.callables[0].param_count, 3);
    assert!(
        doc.effect_sightings
            .iter()
            .any(|e| e.kind == EffectSightingKind::ConsoleOutput)
    );
    Ok(())
}

#[test]
fn symbol_homonymy_tdd_fortress_and_script_exec_guards() -> Result<(), Box<dyn core::error::Error>>
{
    let homonym_rs = "\
pub struct Holder { pub config: u32 }
pub fn read_field(h: &Holder) -> u32 { h.config }
#[test]
fn fake_verify_and_weak() {
    check_something(1);
    verify_other(2);
}
#[test]
fn weak_only_and_reflexive() {
    let res: Result<i32, ()> = Ok(4);
    assert!(res.is_ok());
}
#[test]
fn tautology_expr() {
    let xs = [1, 2, 3];
    assert_eq!(xs.len(), xs.len());
}
";
    let doc = parse_source(&RepoPath::new("src/holder.rs")?, homonym_rs)?;
    assert!(!doc.referenced_symbols.contains("config"));
    assert_eq!(doc.test_cases.len(), 3);
    assert_eq!(doc.test_cases[0].assertion_count, 0);
    assert!(doc.test_cases[1].has_weak_only_assertion);
    assert!(doc.test_cases[2].has_tautology);

    let prod_doc = parse_source(
        &RepoPath::new("src/runner.rs")?,
        "fn f() { Command::new(\"sh\").arg(\"scripts/install.sh\"); }",
    )?;
    assert!(prod_doc.script_refs.is_empty());

    let inert_test = parse_source(
        &RepoPath::new("tests/inert.rs")?,
        "const _FAKE: &str = \"scripts/install.sh\";",
    )?;
    assert!(inert_test.script_refs.is_empty());

    let real_test = parse_source(
        &RepoPath::new("tests/exec.rs")?,
        "fn t() { let s = root.join(\"scripts/install.sh\"); Command::new(\"sh\").arg(&s); }",
    )?;
    assert!(real_test.script_refs.contains("scripts/install.sh"));
    assert!(real_test.script_refs.contains("install.sh"));
    Ok(())
}

#[test]
fn ts_and_python_tautologies_and_weak_assertions() -> Result<(), Box<dyn core::error::Error>> {
    let ts_test = r#"
test("reflexive", () => { expect(a + b).toEqual(a + b); });
test("weak", () => { expect(user).toBeDefined(); expect(user).not.toBeUndefined(); });
"#;
    let ts_doc = parse_source(&RepoPath::new("tests/app.test.ts")?, ts_test)?;
    assert_eq!(ts_doc.test_cases.len(), 2);
    assert!(ts_doc.test_cases[0].has_tautology);
    assert!(ts_doc.test_cases[1].has_weak_only_assertion);

    let py_test = "def test_reflexive():\n    assert x + 1 == x + 1\n\ndef test_weak():\n    assert result is not None\n";
    let py_doc = parse_source(&RepoPath::new("tests/test_mod.py")?, py_test)?;
    assert_eq!(py_doc.test_cases.len(), 2);
    assert!(py_doc.test_cases[0].has_tautology);
    assert!(py_doc.test_cases[1].has_weak_only_assertion);
    Ok(())
}

#[test]
fn detects_project_markers_and_dockerfile_names() -> Result<(), Box<dyn core::error::Error>> {
    assert_eq!(
        detect_language(&RepoPath::new("Dockerfile")?),
        Language::Dockerfile
    );
    assert_eq!(
        detect_project_marker(&RepoPath::new("Cargo.toml")?),
        Some(ProjectMarker::CargoManifest)
    );
    Ok(())
}
