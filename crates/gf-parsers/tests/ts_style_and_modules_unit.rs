//! Direct `gf-parsers` unit tests for `style_and_i18n.rs`, `modules_and_types.rs`,
//! `jsonc.rs`, `tsconfig_jsonc.rs`, and `tool_output_ts.rs` edge cases.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gf_core::{
    Language, RepoPath, StrykerConfigIssue, StyleLeakKind, ToolConfigViolation, TsconfigIssue,
};
use gf_parsers::{
    langs::{jsonc::strip_jsonc_to_json, ts::TsDialect},
    parse_biome_json, parse_source,
};

type Res = Result<(), Box<dyn std::error::Error>>;

const STYLE_PROBE_TS: &str = "\
import logo from \"./logo.svg\";
import dataImport from \"data:image/svg+xml,inline.svg\";
import codeMod from \"./helper.ts\";
const okUrls = [\"https://cdn.example.com/remote.svg\", \"https://cdn.example.com (#fff)\", \"http://example.com (#123456)\"];
const nonMatches = [\"#zzzzzz\", \"#12\", \"px\", \"--px\", \"v2em\", \"not a file.png\", \"png\"];
const okStyleProp = { styleName: \"card\" };
el.styleName = \"card\";
const tplOk = html`<div>safe</div>`;
otherFn(\"ignored.key\");
t(121);
t(\"dyn.\" + suffix);
t(\"\");
t(\"home.title\");
i18n.translate('nav.back');
const k = t(`cta.go`);
const assetA = \"/img/hero.webp?v=1#top\";
const assetB = `/assets/bg.png`;
const dataUri = \"data:image/png;base64,AAA\";
const hexBorder = \"1px solid #abc\";
const hexProp = \"color:#def\";
const rgbCol = \"rgb(1, 2, 3)\";
const pxVal = \"16px\";
const negRem = \"-0.5rem\";
styled.div(\"color: red\");
styled(Comp)(\"padding: 0\");
css(\"margin: 0\");
createGlobalStyle(\"body {}\");
const s1 = styled.span`color: red`;
const s2 = styled(Comp)`color: blue`;
const s3 = css`display: flex`;
const s4 = createGlobalStyle`html {}`;
el.style.width = \"100%\";
el.style = \"display:none\";
";

const MOD_EDGES_TS: &str = "\
import \"./polyfill.js\";
import Def from \"./def.js\";
import * as Ns from \"./ns.js\";
import type * as TypeNs from \"./tns.js\";
import { type } from \"./kw_val.js\";
import { type OnlyT } from \"./only_t.js\";
import { type T1, liveVal } from \"./mixed.js\";
import Def2, { type T2 } from \"./def_mixed.js\";
export { alpha, type Beta as BetaOut } from \"./re_mix.js\";
export { type PureRe } from \"./re_pure.js\";
export type { GroupRe } from \"./re_grp.js\";
export {} from \"\";
export function sigOnly(x: number): number;
export const valExport = 1;
export const { ignoredDestruct } = srcObj;
export interface PureMerged { a: number; }
export interface PureMerged { b: string; }
export type ValueMerged = string;
export const ValueMerged = \"v\";
const d1 = import(\"./dyn_ok.js\");
const d2 = import(`./${dynName}.js`);
declare module \"amb\" { export {} from \"\"; export { X } from \"./amb_re.js\"; }
";

#[test]
fn style_leaks_colors_css_units_assets_and_translations_are_exact() -> Res {
    let doc = parse_source(&RepoPath::new("src/style_probe.ts")?, STYLE_PROBE_TS)?;
    assert_eq!(
        doc.asset_refs,
        BTreeSet::from([
            String::from("./logo.svg"),
            String::from("/assets/bg.png"),
            String::from("/img/hero.webp"),
        ])
    );
    assert_eq!(
        doc.translation_keys_used,
        BTreeSet::from([
            String::from("cta.go"),
            String::from("home.title"),
            String::from("nav.back"),
        ])
    );
    let kinds: Vec<StyleLeakKind> = doc.style_leaks.iter().map(|s| s.kind).collect();
    let mut expected = vec![
        StyleLeakKind::DataUri,
        StyleLeakKind::RawColorLiteral,
        StyleLeakKind::RawColorLiteral,
        StyleLeakKind::RawColorLiteral,
        StyleLeakKind::RawCssUnit,
        StyleLeakKind::RawCssUnit,
    ];
    expected.extend(core::iter::repeat_n(StyleLeakKind::CssInJs, 10));
    assert_eq!(kinds, expected);
    let broken_quote = parse_source(&RepoPath::new("src/unterm.ts")?, "t(\"unterm);\n")?;
    assert_eq!(broken_quote.translation_keys_used, BTreeSet::new());
    Ok(())
}

#[test]
fn ts_module_imports_reexports_and_declaration_merging_edges_are_exact() -> Res {
    let doc = parse_source(&RepoPath::new("src/mod_edges.ts")?, MOD_EDGES_TS)?;
    let imp_summary: Vec<(&str, bool, Vec<&str>)> = doc
        .imports
        .iter()
        .map(|i| {
            (
                i.module_source.as_str(),
                i.is_type_only,
                i.symbols.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        imp_summary,
        vec![
            ("./polyfill.js", false, vec![]),
            ("./def.js", false, vec!["Def"]),
            ("./ns.js", false, vec![]),
            ("./tns.js", true, vec![]),
            ("./kw_val.js", false, vec!["type"]),
            ("./only_t.js", true, vec!["OnlyT"]),
            ("./mixed.js", false, vec!["T1", "liveVal"]),
            ("./def_mixed.js", false, vec!["Def2", "T2"]),
            ("./re_mix.js", false, vec!["alpha", "Beta"]),
            ("./re_pure.js", true, vec!["PureRe"]),
            ("./re_grp.js", true, vec!["GroupRe"]),
            ("./dyn_ok.js", false, vec![]),
            ("./amb_re.js", true, vec![]),
        ]
    );
    let exp_summary: Vec<(&str, bool)> = doc
        .exports
        .iter()
        .map(|e| (e.symbol.as_str(), e.is_type_only))
        .collect();
    assert_eq!(
        exp_summary,
        vec![
            ("alpha", false),
            ("BetaOut", true),
            ("PureRe", true),
            ("GroupRe", true),
            ("valExport", false),
            ("PureMerged", true),
            ("ValueMerged", false),
        ]
    );
    Ok(())
}

#[test]
fn jsonc_and_stryker_config_edges_are_exact() -> Res {
    assert_eq!(strip_jsonc_to_json("1 / 2 * 3"), "1 / 2 * 3");
    let raw_jsonc =
        "{\n  /* star * mid \n end */\n  \"slash\": \"a/b\",\n  \"esc\": \"quote \\\" , }\",\n}\n";
    let val: serde_json::Value = serde_json::from_str(&strip_jsonc_to_json(raw_jsonc))?;
    assert_eq!(
        (val["slash"].as_str(), val["esc"].as_str()),
        (Some("a/b"), Some("quote \" , }"))
    );
    let ok_stryker = parse_source(
        &RepoPath::new("stryker.config.json")?,
        "{\"mutate\":[\"src/**/*.ts\"],\"thresholds\":{\"break\":85}}",
    )?;
    assert_eq!(ok_stryker.tool_config_findings, Vec::new());
    for bad_break in ["null", "0"] {
        let s = parse_source(
            &RepoPath::new("stryker.config.json")?,
            &format!("{{\"mutate\":[],\"thresholds\":{{\"break\":{bad_break}}}}}"),
        )?;
        let vs: Vec<_> = s.tool_config_findings.iter().map(|f| f.violation).collect();
        assert_eq!(
            vs,
            vec![
                ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::EmptyMutate),
                ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::DisabledBreakThreshold),
            ]
        );
    }
    Ok(())
}

#[test]
fn tsconfig_paths_biome_fallback_and_dialect_edges_are_exact() -> Res {
    let ts_empty_ext = parse_source(
        &RepoPath::new("tsconfig.json")?,
        "{\"extends\":\"   \",\"compilerOptions\":{\"baseUrl\":\"   \",\"paths\":{\"@a/*\":[\" src/a/* \",\"  \"],\"@empty\":[\" \"],\"@bad\":1}}}",
    )?;
    assert_eq!(
        ts_empty_ext.tool_config_findings[0].violation,
        ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict)
    );
    let sum = ts_empty_ext.tsconfig.as_ref().ok_or("missing tsconfig")?;
    assert_eq!(
        (sum.extends.as_deref(), sum.base_url.as_deref()),
        (None, None)
    );
    assert_eq!(
        sum.paths,
        BTreeMap::from([(String::from("@a/*"), vec![String::from("src/a/*")])])
    );
    let ts_arr_ext = parse_source(
        &RepoPath::new("tsconfig.app.json")?,
        "{\"extends\":[\"  \",\"./base.json\",\"./leaf.json\"],\"compilerOptions\":{\"strict\":true,\"baseUrl\":\"./src\"}}",
    )?;
    let arr_sum = ts_arr_ext.tsconfig.as_ref().ok_or("missing tsconfig")?;
    assert_eq!(
        (arr_sum.extends.as_deref(), arr_sum.base_url.as_deref()),
        (Some("./leaf.json"), Some("./src"))
    );
    let bm = parse_biome_json(
        "{\"diagnostics\":[{\"severity\":\"error\",\"description\":\"   \",\"message\":[{\"content\":\"Markup msg\"}],\"location\":{\"path\":\"src/x.ts\"}}]}",
        Path::new("/repo"),
    )?;
    assert_eq!(bm[0].message, "Markup msg");
    assert_eq!(
        TsDialect::from_path_and_lang(&RepoPath::new("README.md")?, Language::Markdown),
        None
    );
    assert_eq!(
        TsDialect::from_path_and_lang(&RepoPath::new("src/a.js")?, Language::JavaScript),
        Some(TsDialect::JavaScript)
    );
    assert_eq!(
        TsDialect::from_path_and_lang(&RepoPath::new("src/a.jsx")?, Language::Jsx),
        Some(TsDialect::Jsx)
    );
    assert!(!TsDialect::TypeScript.supports_jsx());
    assert!(TsDialect::Tsx.supports_jsx());
    Ok(())
}
