//! Symmetric corpus tests for dissected Oxc / Biome / React 19 / `tree-sitter-typescript` quirks:
//! `.ts` vs `.tsx` generics, `this` fake param, overloads, constructor parameter properties,
//! `.d.ts` ambient blocks, TS type/async safety (`GF080`), `@ts-expect-error` (`GF070`),
//! and React 19 `use()` / early-`return` Rules of Hooks.

mod helpers;

use gf_core::{
    Config, DomainKind, KissYagniKind, LangRuleFinding, RepoPath, ToolConfigViolation,
    TsconfigIssue, UastDocument,
};
use gf_parsers::{langs::ts::TsDialect, langs::ts::TypeScriptExtractor, parse_source};
use gf_rules::evaluate_document;
use helpers::{
    BAD_SAFETY_TS, CLEAN_SAFETY_TS, INVALID_REACT19_TSX, OVERLOADS_AND_CTOR_TS, VALID_REACT19_TSX,
};

type TestRes = Result<(), Box<dyn std::error::Error>>;

fn parse_rel(rel: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel)?, src)?)
}

fn lang_codes_and_lines(doc: &UastDocument) -> Vec<(&'static str, u32)> {
    doc.lang_rule_findings
        .iter()
        .map(|f| (LangRuleFinding::code(f), f.span.start_line()))
        .collect()
}

#[test]
fn extension_routing_separates_ts_generics_from_tsx_and_mjs_jsx() -> TestRes {
    for ext in ["ts", "mts", "cts"] {
        let d = parse_rel(
            &format!("src/id.{ext}"),
            "export const id = <T>(x: T): T => x;\nexport const cast = <string>raw;\n",
        )?;
        assert_eq!(d.syntax_errors, Vec::new());
    }
    let ts_rejects_jsx = parse_rel("src/no_jsx.ts", "export const W = () => <div />;\n")?;
    assert_ne!(ts_rejects_jsx.syntax_errors.len(), 0);
    let tsx_comma = parse_rel("src/id.tsx", "export const id = <T,>(x: T): T => x;\n")?;
    assert_eq!(tsx_comma.syntax_errors, Vec::new());
    let tsx_broken = parse_rel("src/broken.tsx", "export const cast = <string>raw;\n")?;
    assert_ne!(tsx_broken.syntax_errors.len(), 0);
    for ext in ["tsx", "jsx", "js", "mjs", "cjs"] {
        let jsx_ok = parse_rel(
            &format!("src/widget.{ext}"),
            "export const W = () => <div className=\"x\" />;\n",
        )?;
        assert_eq!(jsx_ok.syntax_errors, Vec::new());
    }
    Ok(())
}

#[test]
fn overloads_this_param_ctor_props_and_ambient_dts_obey_invariants() -> TestRes {
    let cfg = Config::strict_defaults();
    let this_ok = parse_rel(
        "src/this_ok.ts",
        "export function handler(this: Window, a: number, b: number, c: number, d: number): number {\n  return a + b + c + d;\n}\nexport function flagFn(this: boolean, active: boolean): boolean {\n  return active;\n}\n",
    )?;
    assert_eq!(this_ok.callables[0].param_count, 4);
    assert_eq!(evaluate_document(&this_ok, &cfg), Vec::new());

    let five_bad = parse_rel(
        "src/five.ts",
        "export function tooMany(a: number, b: number, c: number, d: number, e: number): number {\n  return a + b + c + d + e;\n}\n",
    )?;
    assert!(
        evaluate_document(&five_bad, &cfg)
            .iter()
            .any(|d| d.rule_id == "GF003")
    );

    let ov_doc = parse_rel("src/store.ts", OVERLOADS_AND_CTOR_TS)?;
    let fn_names: Vec<&str> = ov_doc.callables.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(fn_names, vec!["parseVal", "fetch", "constructor"]);
    assert_eq!(ov_doc.types[0].field_count, 4);
    assert_eq!(ov_doc.types[0].bool_field_count, 3);
    assert_eq!(ov_doc.types[0].method_count, 2);
    assert!(
        evaluate_document(&ov_doc, &cfg)
            .iter()
            .any(|d| d.rule_id == "GF009")
    );

    let dts = parse_rel(
        "src/types/global.d.ts",
        "declare global {\n  function legacyGlobal(x: any): any;\n}\nexport declare function ambientFn(a: string): void;\n",
    )?;
    assert_eq!(dts.callables, Vec::new());
    assert_eq!(dts.lang_rule_findings, Vec::new());
    Ok(())
}

#[test]
fn ts_safety_symmetric_pairs_and_ts_expect_error_precision() -> TestRes {
    let clean_doc = parse_rel("src/safe.ts", CLEAN_SAFETY_TS)?;
    assert_eq!(lang_codes_and_lines(&clean_doc), Vec::new());
    assert_eq!(clean_doc.bypass_findings, Vec::new());

    let bad_doc = parse_rel("src/hazards.ts", BAD_SAFETY_TS)?;
    assert_eq!(
        lang_codes_and_lines(&bad_doc),
        vec![
            ("TS-NO-EXPLICIT-ANY", 1),
            ("TS-NO-DOUBLE-ASSERTION", 2),
            ("TS-NO-DOUBLE-ASSERTION", 3),
            ("TS-NO-NON-NULL-OPTIONAL-CHAIN", 4),
            ("TS-NO-NON-NULL-OPTIONAL-CHAIN", 5),
            ("TS-NO-NON-NULL-OPTIONAL-CHAIN", 6),
            ("TS-NO-ASYNC-PROMISE-EXECUTOR", 7),
            ("TS-NO-CONSOLE-DEBUG", 8),
            ("TS-NO-CONSOLE-DEBUG", 9),
        ]
    );
    let bypass_lines: Vec<u32> = bad_doc
        .bypass_findings
        .iter()
        .map(|b| b.span.start_line())
        .collect();
    assert_eq!(bypass_lines, vec![12, 13, 14]);

    let mut cfg = Config::strict_defaults();
    cfg.entrypoints = vec![String::from("src/server.ts")];
    let srv_doc = parse_rel(
        "src/server.ts",
        "export function boot(): void {\n  console.info(\"up\");\n  debugger;\n}\n",
    )?;
    let srv_diags = evaluate_document(&srv_doc, &cfg);
    assert_eq!(srv_diags.len(), 1);
    assert_eq!(srv_diags[0].span.start_line(), 3);

    let script_doc = TypeScriptExtractor::new(TsDialect::TypeScript).extract_in_domain(
        &RepoPath::new("scripts/gen.ts")?,
        "const x: any = 1;\nconsole.log(x);\n",
        DomainKind::Scripts,
    )?;
    assert_eq!(script_doc.lang_rule_findings, Vec::new());
    Ok(())
}

#[test]
fn react19_use_exemption_early_return_hooks_and_unstable_nested_components() -> TestRes {
    let ok_doc = parse_rel("src/ValidHooks.tsx", VALID_REACT19_TSX)?;
    assert_eq!(lang_codes_and_lines(&ok_doc), Vec::new());

    let bad_doc = parse_rel("src/BrokenHooks.tsx", INVALID_REACT19_TSX)?;
    assert_eq!(
        lang_codes_and_lines(&bad_doc),
        vec![
            ("REACT-RULES-OF-HOOKS", 2),
            ("REACT-RULES-OF-HOOKS", 4),
            ("TS-NO-ASYNC-EFFECT", 7),
            ("REACT-RULES-OF-HOOKS", 11),
            ("REACT-RULES-OF-HOOKS", 14),
            ("REACT-RULES-OF-HOOKS", 15),
            ("REACT-NO-UNSTABLE-NESTED-COMPONENT", 16),
            ("REACT-NO-UNSTABLE-NESTED-COMPONENT", 19),
            ("REACT-RULES-OF-HOOKS", 20),
        ]
    );
    Ok(())
}

#[test]
fn jsonc_tsconfig_and_biome_config_preserve_urls_and_detect_weakenings() -> TestRes {
    let ok_tsconfig = "{\n  // Docs: https://www.typescriptlang.org/tsconfig#strict\n  \"extends\": [\"@tsconfig/strictest\"],\n  \"compilerOptions\": {\n    \"baseUrl\": \"./src\", /* block comment */\n    \"paths\": { \"@core/*\": [\"core/*\"], },\n  },\n}\n";
    let ok_doc = parse_rel("tsconfig.json", ok_tsconfig)?;
    assert_eq!(ok_doc.syntax_errors, Vec::new());
    assert_eq!(ok_doc.tool_config_findings, Vec::new());

    let bad_tsconfig = "{\n  \"extends\": \"@tsconfig/strictest\",\n  \"compilerOptions\": {\n    \"strict\": false,\n    \"noImplicitAny\": false,\n    \"strictNullChecks\": false,\n  },\n}\n";
    let bad_doc = parse_rel("tsconfig.app.json", bad_tsconfig)?;
    let issues: Vec<_> = bad_doc
        .tool_config_findings
        .iter()
        .map(|f| f.violation)
        .collect();
    assert_eq!(
        issues,
        vec![
            ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict),
            ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::NoImplicitAnyFalse),
            ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::StrictNullChecksFalse),
        ]
    );

    let bad_biome =
        "{\n  // https://biomejs.dev\n  \"linter\": {\n    \"enabled\": false,\n  },\n}\n";
    let biome_doc = parse_rel("biome.jsonc", bad_biome)?;
    assert_eq!(
        biome_doc.tool_config_findings[0].violation,
        ToolConfigViolation::BiomeLinterDisabled
    );
    assert_eq!(biome_doc.tool_config_findings[0].span.start_line(), 4);
    assert_eq!(
        bad_doc
            .kiss_yagni_findings
            .iter()
            .filter(|k| k.kind == KissYagniKind::TodoMarker)
            .count(),
        0
    );
    Ok(())
}
