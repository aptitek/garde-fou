//! Realistic multi-language fixtures testing clean vs dirty code and syntax errors.

use gf_core::{Config, Language, RepoPath};
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
    assert_eq!(doc.callables[0].span.start_line(), 9);
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
    let broken_diags = evaluate_document(&broken_doc, &Config::strict_defaults());
    assert!(broken_diags.iter().any(|d| d.rule_id == "GF000"));
    Ok(())
}

#[test]
fn typescript_and_tsx_god_class_and_arrows() -> Result<(), Box<dyn core::error::Error>> {
    let ts_src = r#"import { alpha } from "./alpha";
import { beta } from "./beta";

export interface Repo {
    id: string;
    load(): void;
}

export const compute = (x: number, y: number): number => {
    if (x > 0 && y > 0) {
        return x ? y : 0;
    }
    return x ?? 0;
};

export class GodController {
    m1() {} m2() {} m3() {} m4() {} m5() {} m6() {}
    m7() {} m8() {} m9() {} m10() {} m11() {}
}
"#;
    let doc = parse_source(&RepoPath::new("web/app.ts")?, ts_src)?;
    assert_eq!(doc.language, Language::TypeScript);
    assert_eq!(doc.imports.len(), 2);
    assert!(doc.exports.len() >= 2);
    let god = doc.types.iter().find(|t| t.name == "GodController");
    assert_eq!(god.map(|t| t.method_count), Some(11));

    let diags = evaluate_document(&doc, &Config::strict_defaults());
    assert!(diags.iter().any(|d| d.rule_id == "GF008"));

    let tsx_doc = parse_source(
        &RepoPath::new("web/View.tsx")?,
        "export const View = () => <div className=\"ok\">hi</div>;\n",
    )?;
    assert_eq!(tsx_doc.language, Language::Tsx);
    assert_eq!(tsx_doc.syntax_errors, Vec::new());
    Ok(())
}

#[test]
fn python_comprehensions_elif_and_markers() -> Result<(), Box<dyn core::error::Error>> {
    let py_src = r"import os
from pathlib import Path

class Analyzer:
    version = 1

    def evaluate(self, items, threshold):
        filtered = [x for x in items if x > 0 and x < threshold]
        if not filtered:
            return 0
        elif len(filtered) == 1:
            return filtered[0]
        else:
            return sum(filtered)
";
    let doc = parse_source(&RepoPath::new("pkg/analyzer.py")?, py_src)?;
    assert_eq!(doc.language, Language::Python);
    assert_eq!(doc.imports.len(), 2);
    assert_eq!(doc.exports.len(), 1);
    assert_eq!(doc.types[0].field_count, 1);
    assert_eq!(doc.types[0].method_count, 1);
    assert_eq!(doc.callables[0].param_count, 2);
    assert!(doc.callables[0].cyclomatic_complexity >= 5);
    assert_eq!(
        evaluate_document(&doc, &Config::strict_defaults()),
        Vec::new()
    );

    assert_eq!(
        detect_language(&RepoPath::new("Dockerfile")?),
        Language::Dockerfile
    );
    assert_eq!(detect_language(&RepoPath::new("index.jsx")?), Language::Jsx);
    assert_eq!(
        detect_language(&RepoPath::new("index.js")?),
        Language::JavaScript
    );
    assert_eq!(
        detect_project_marker(&RepoPath::new("Cargo.toml")?),
        Some(ProjectMarker::CargoManifest)
    );
    Ok(())
}

#[test]
fn rust_enums_traits_matches_closures_and_unsafe_blocks() -> Result<(), Box<dyn core::error::Error>>
{
    let src = r"pub enum Mode { Fast, Strict }

pub trait Runner {
    fn execute(&self) -> i32;
}

pub fn dispatch(m: Mode, x: i32) -> i32 {
    let adjust = |v: i32| {
        if v > 10 { v - 1 } else if v < 0 { 0 } else { v }
    };
    unsafe {
        match m {
            Mode::Fast => adjust(x),
            Mode::Strict => adjust(x + 1),
        }
    }
}
";
    let doc = parse_source(&RepoPath::new("src/dispatch.rs")?, src)?;
    assert_eq!(doc.types.len(), 2);
    assert_eq!(doc.types[1].method_count, 1);
    assert_eq!(doc.callables.len(), 2);
    let diags = evaluate_document(&doc, &Config::strict_defaults());
    assert!(diags.iter().any(|d| d.rule_id == "GF010"));
    Ok(())
}
