//! Mutation-hardening edge tests for `callables.rs`, `jsx_dangerous.rs`, `jsx_keys.rs`,
//! `jsx_semantics.rs`, `react_classify.rs`, `react_hooks.rs`, `routing.rs`, and `ts_safety.rs`.

use gf_core::{DomainKind, LangRuleFinding, RepoPath, StyleLeakKind, UiCheckKind};
use gf_parsers::{
    langs::ts::{TsDialect, TypeScriptExtractor},
    parse_source,
};

type Res = Result<(), Box<dyn std::error::Error>>;

fn codes(src: &str, ext: &str) -> Result<Vec<&'static str>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(&format!("src/Probe.{ext}"))?, src)?;
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect())
}

#[test]
fn callables_ctor_plain_params_bool_fields_and_interface_counts_are_exact() -> Res {
    let src = concat!(
        "export class Widget {\n",
        "  count: number = 0;\n",
        "  enabled: boolean = true;\n",
        "  constructor(public overloaded: string);\n",
        "  constructor(plain: string, plainFlag: boolean, readonly pinned: boolean) {}\n",
        "  helper(active: boolean): number { return active ? this.count : 0; }\n",
        "}\n",
        "export interface Spec {\n",
        "  id: string;\n",
        "  ready: boolean;\n",
        "  run(): void;\n",
        "}\n",
    );
    let doc = parse_source(&RepoPath::new("src/widget.ts")?, src)?;
    assert_eq!(doc.types[0].field_count, 3);
    assert_eq!(doc.types[0].bool_field_count, 2);
    assert_eq!(doc.types[0].method_count, 2);
    assert_eq!(doc.types[1].field_count, 2);
    assert_eq!(doc.types[1].bool_field_count, 1);
    assert_eq!(doc.types[1].method_count, 1);

    let ts_no_jsx = parse_source(
        &RepoPath::new("src/math.ts")?,
        "export const cmp = <div style={{ color: 1 }} />;\n",
    )?;
    assert!(
        !ts_no_jsx
            .style_leaks
            .iter()
            .any(|s| s.kind == StyleLeakKind::InlineStyleAttr)
    );
    Ok(())
}

#[test]
fn clean_jsx_custom_tags_decorative_imgs_and_labeled_buttons_pass() -> Res {
    let clean_src = concat!(
        "export function CleanEdges({ fn, dynTitle }: { fn: () => void; dynTitle: string }) {\n",
        "  return (<section>\n",
        "    <Menu.Item /><svg:path /><my-element onClick={fn} />\n",
        "    <a href=\"https://example.com\" target=\"_self\">{t(\"ok\")}</a>\n",
        "    <div title={dynTitle} />\n",
        "    <img src=\"/a.svg\" alt=\"\" aria-hidden={true} />\n",
        "    <button type=\"button\" aria-label={t(\"close\")} />\n",
        "    <button type=\"button\" aria-labelledby=\"hdr\" />\n",
        "  </section>);\n}\n",
    );
    let clean_doc = parse_source(&RepoPath::new("src/CleanEdges.tsx")?, clean_src)?;
    assert_eq!(clean_doc.ui_checks, Vec::new());
    assert_eq!(clean_doc.style_leaks, Vec::new());
    assert_eq!(clean_doc.lang_rule_findings, Vec::new());
    Ok(())
}

#[test]
fn dirty_jsx_inline_style_svg_buttons_and_incomplete_click_a11y_fail() -> Res {
    let dirty_src = concat!(
        "export function DirtyEdges({ fn }: { fn: () => void }) {\n",
        "  return (<section>\n",
        "    <div style={{ opacity: 1 }} placeholder=\"Enter email\" />\n",
        "    <style>{\".x {}\"}</style><svg><path /></svg>\n",
        "    <img src=\"/b.svg\" alt=\"\" aria-hidden=\"false\" />\n",
        "    <img src=\"/c.svg\" alt=\"\" role=\"button\" />\n",
        "    <button type=\"button\" /><button type=\"button\">   </button>\n",
        "    <div onClick={fn} role=\"button\" /><div onClick={fn} tabIndex={0} />\n",
        "  </section>);\n}\n",
    );
    let dirty_doc = parse_source(&RepoPath::new("src/DirtyEdges.tsx")?, dirty_src)?;
    let leak_kinds: Vec<StyleLeakKind> = dirty_doc.style_leaks.iter().map(|s| s.kind).collect();
    assert_eq!(
        leak_kinds,
        vec![
            StyleLeakKind::InlineStyleAttr,
            StyleLeakKind::StyleElement,
            StyleLeakKind::InlineSvg,
        ]
    );
    let ui_kinds: Vec<UiCheckKind> = dirty_doc.ui_checks.iter().map(|u| u.kind).collect();
    assert_eq!(
        ui_kinds,
        vec![
            UiCheckKind::UntranslatedText,
            UiCheckKind::ImgMissingAlt,
            UiCheckKind::ImgMissingAlt,
            UiCheckKind::ButtonMissingLabel,
            UiCheckKind::ButtonMissingLabel,
            UiCheckKind::NonInteractiveClick,
            UiCheckKind::NonInteractiveClick,
        ]
    );
    Ok(())
}

#[test]
fn jsx_iterator_keys_block_callbacks_destructured_second_param_and_spread_edges() -> Res {
    let ok_keys = concat!(
        "export function OkKeys({ xs }: { xs: { id: string }[] }) {\n",
        "  return (<ul>\n",
        "    {xs.map((it, { flag }: { flag: boolean }) => {\n",
        "      const badge = () => <span>•</span>;\n",
        "      return <li key={it.id}>{badge()}{String(flag)}</li>;\n",
        "    })}\n",
        "  </ul>);\n}\n",
    );
    assert_eq!(codes(ok_keys, "tsx")?, Vec::<&str>::new());

    let bad_keys = concat!(
        "export function BadKeys({ xs, key }: { xs: { id: string }[]; key: Record<string, string> }) {\n",
        "  return (<ul>\n",
        "    {xs.map((it, $i_0) => <li key={$i_0}>{it.id}</li>)}\n",
        "    {xs.map((it) => <li className=\"row\" {...key}>{it.id}</li>)}\n",
        "    {xs.map((it) => <>{it.id}</>)}\n",
        "    {xs.map((it) => { return <li>{it.id}</li>; })}\n",
        "    {xs.map((it) => it.id ? <li>{it.id}</li> : <li key={it.id}>{it.id}</li>)}\n",
        "    {xs.map((it) => Boolean(it.id) && <li>{it.id}</li>)}\n",
        "  </ul>);\n}\n",
    );
    let bad_doc = parse_source(&RepoPath::new("src/BadKeys.tsx")?, bad_keys)?;
    assert_eq!(bad_doc.lang_rule_findings.len(), 6);
    assert!(
        bad_doc.lang_rule_findings[2]
            .message
            .contains("shorthand Fragment")
    );
    assert!(
        bad_doc.lang_rule_findings[1]
            .message
            .contains("missing a `key`")
    );
    Ok(())
}

#[test]
fn valid_react_classifiers_null_components_and_private_methods_pass() -> Res {
    let valid_src = concat!(
        "import { useMemo, useState, useEffect } from \"react\";\n",
        "export function PlainPascalHelper(n: number) { if (n > 0) { return { n }; } return 0; }\n",
        "export const ArrowMath = (n: number) => n + 1;\n",
        "export const NullArrow = () => null;\n",
        "export const useExprHook = () => useMemo(() => 1, []);\n",
        "let AssignedComp: unknown;\n",
        "AssignedComp = () => { const [a] = useState(0); const step = a + 1; const [b] = useState(step); return <div>{b}</div>; };\n",
        "export function NullReturnComp({ hide }: { hide: boolean }) {\n",
        "  const sum = 1 + useMemo(() => 2, []);\n",
        "  useEffect(() => { VoidCleanup(sum); }, [sum]);\n",
        "  runAsyncJob(async () => { await Promise.resolve(sum); });\n",
        "  function LocalPascalHelper() { return 42; }\n",
        "  const LocalArrowMath = () => 42;\n",
        "  getRegistry().useState();\n",
        "  React[dynKey]();\n",
        "  class Box { #usePriv() { return 1; } run(b: Box) { return b.#usePriv(); } }\n",
        "  const p = new Promise((resolve) => { resolve(LocalPascalHelper() + LocalArrowMath()); });\n",
        "  const c = getFn().prop!;\n",
        "  if (hide || Boolean(p) || Boolean(c)) { return null; }\n",
        "  return null;\n",
        "}\n",
    );
    assert_eq!(codes(valid_src, "tsx")?, Vec::<&str>::new());
    Ok(())
}

#[test]
fn isolated_conditional_hooks_nested_null_and_optional_call_chains_fail() -> Res {
    let isolated_bad = concat!(
        "import React, { useState } from \"react\";\n",
        "export function CondTernary({ c }: { c: boolean }) { const x = c ? React.useState(0) : 0; return <div>{String(x)}</div>; }\n",
        "export function CondOr({ c }: { c: boolean }) { const x = c || useState(0); return <div>{String(x)}</div>; }\n",
        "export const useCondArrow = (c: boolean) => c && useState(0);\n",
        "export function LoopHook({ xs }: { xs: number[] }) { for (const x of xs) { useState(x); } const NestedNull = () => null; return <NestedNull />; }\n",
        "export function AfterThrow({ bad }: { bad: boolean }) { if (bad) { throw new Error(\"e\"); } const [v] = useState(0); return <div>{v}</div>; }\n",
        "export function OptCallChain(fn?: () => { p?: string }, xs?: string[]): string { return fn?.().p! + xs?.[0]!; }\n",
    );
    assert_eq!(
        codes(isolated_bad, "tsx")?,
        vec![
            "REACT-RULES-OF-HOOKS",
            "REACT-RULES-OF-HOOKS",
            "REACT-RULES-OF-HOOKS",
            "REACT-NO-UNSTABLE-NESTED-COMPONENT",
            "REACT-RULES-OF-HOOKS",
            "REACT-RULES-OF-HOOKS",
            "TS-NO-NON-NULL-OPTIONAL-CHAIN",
            "TS-NO-NON-NULL-OPTIONAL-CHAIN",
        ]
    );
    let script_dbg = TypeScriptExtractor::new(TsDialect::TypeScript).extract_in_domain(
        &RepoPath::new("scripts/tool.ts")?,
        "debugger;\n",
        DomainKind::Scripts,
    )?;
    assert_eq!(script_dbg.lang_rule_findings, Vec::new());
    Ok(())
}
