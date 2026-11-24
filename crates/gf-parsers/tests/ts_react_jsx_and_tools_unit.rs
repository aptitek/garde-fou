//! Symmetric unit tests for JSX `a11y` (`GF066` intrinsic vs `PascalCase`), `i18n` (`GF062`),
//! `REACT-JSX-KEY`, `REACT-NO-DANGEROUS-JSX`, `is_hook_identifier`, and Tier 3 parsers
//! (`parse_oxlint_json`, `parse_biome_json`, `parse_tsc_output`).

use std::path::Path;

use gf_core::{ExternalToolKind, LangRuleFinding, RepoPath, UiCheckKind};
use gf_parsers::{
    is_hook_identifier, parse_biome_json, parse_oxlint_json, parse_source, parse_tsc_output,
};

type TestRes = Result<(), Box<dyn std::error::Error>>;

#[test]
fn hook_identifier_classifier_rejects_false_friends() {
    for valid in ["use", "useState", "use2FA", "use9", "useZ"] {
        assert!(is_hook_identifier(valid), "{valid} must be a hook");
    }
    for invalid in [
        "",
        "u",
        "us",
        "useful",
        "use_effect",
        "use-x",
        "_useHook",
        "myUseHook",
    ] {
        assert!(!is_hook_identifier(invalid), "{invalid} must NOT be a hook");
    }
}

#[test]
fn clean_jsx_exempts_pascal_components_decorative_images_and_punctuation() -> TestRes {
    let clean_jsx = "\
export function CleanView({ items, save }: { items: { id: string; idx: string }[]; save: () => void }) {
  return (
    <section>
      <Button onClick={save}>{t(\"btn.save\")}</Button>
      <Image asset=\"/hero.webp\" />
      <Box onClick={save} />
      <img src=\"/decor.svg\" alt=\"\" aria-hidden=\"true\" />
      <img src=\"/divider.svg\" alt=\"\" role=\"presentation\" />
      <button type=\"button\" onClick={save}>{t(\"btn.ok\")}</button>
      <div role=\"button\" tabIndex={0} onClick={save}>{t(\"btn.div\")}</div>
      <span>• 42 / 100 | → ×</span>
      <code>const rawTechnicalIdentifier = 1;</code>
      <a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer\">{t(\"nav.ext\")}</a>
      <ul>{items.map((item, idx) => <li key={item.idx}>{idx}</li>)}</ul>
    </section>
  );
}
";
    let clean_doc = parse_source(&RepoPath::new("src/CleanView.tsx")?, clean_jsx)?;
    assert_eq!(clean_doc.ui_checks, Vec::new());
    assert_eq!(clean_doc.lang_rule_findings, Vec::new());
    Ok(())
}

const DIRTY_VIEW_TSX: &str = "\
export function DirtyView({ items, save }: { items: { id: string }[]; save: () => void }) {
  return (<section>
    <button onClick={save}>{t(\"btn.save\")}</button>
    <div onClick={save}>{t(\"btn.div\")}</div>
    <img src=\"/missing.png\" />
    <img src=\"/nondecor.png\" alt=\"\" />
    <span>Untranslated label</span>
    <div dangerouslySetInnerHTML={{ __html: \"<b>x</b>\" }} />
    <a href={\"  JaVaScRiPt:\\talert(1)\"}>{t(\"nav.xss\")}</a>
    <a href=\"https://example.com\" target=\"_blank\">{t(\"nav.tabnab\")}</a>
    <ul>{items.map((it) => <li>{it.id}</li>)}</ul>
    <ul>{items.flatMap((it) => <>{it.id}</>)}</ul>
    <ul>{Array.from(items, (it, idx) => <li key={`row-${idx}`}>{it.id}</li>)}</ul>
  </section>);
}
";

#[test]
fn dirty_jsx_flags_intrinsic_a11y_dangerous_props_and_iterable_keys() -> TestRes {
    let dirty_doc = parse_source(&RepoPath::new("src/DirtyView.tsx")?, DIRTY_VIEW_TSX)?;
    let ui_kinds_lines: Vec<(UiCheckKind, u32)> = dirty_doc
        .ui_checks
        .iter()
        .map(|u| (u.kind, u.span.start_line()))
        .collect();
    assert_eq!(
        ui_kinds_lines,
        vec![
            (UiCheckKind::ButtonMissingType, 3),
            (UiCheckKind::NonInteractiveClick, 4),
            (UiCheckKind::ImgMissingAlt, 5),
            (UiCheckKind::ImgMissingAlt, 6),
            (UiCheckKind::UntranslatedText, 7),
        ]
    );
    let react_codes_lines: Vec<(&str, u32)> = dirty_doc
        .lang_rule_findings
        .iter()
        .map(|f| (LangRuleFinding::code(f), f.span.start_line()))
        .collect();
    assert_eq!(
        react_codes_lines,
        vec![
            ("REACT-NO-DANGEROUS-JSX", 8),
            ("REACT-NO-DANGEROUS-JSX", 9),
            ("REACT-NO-DANGEROUS-JSX", 10),
            ("REACT-JSX-KEY", 11),
            ("REACT-JSX-KEY", 12),
            ("REACT-JSX-KEY", 13),
        ]
    );
    Ok(())
}

#[test]
fn oxlint_json_parser_supports_wrapped_and_bare_arrays() -> TestRes {
    let win_root = Path::new("C:\\repo");
    let oxlint_wrapped = r#"{
      "diagnostics": [
        {
          "message": "Unexpected debugger statement",
          "code": "eslint(no-debugger)",
          "severity": "warning",
          "help": "Remove the debugger statement",
          "filename": "C:\\repo\\src\\app.tsx",
          "labels": [{ "span": { "line": 12, "column": 5 } }]
        },
        { "message": "Allowed rule", "severity": "off", "filename": "src/app.tsx" }
      ]
    }"#;
    let ox = parse_oxlint_json(oxlint_wrapped, win_root)?;
    assert_eq!(ox.len(), 1);
    assert_eq!(ox[0].tool, ExternalToolKind::Oxlint);
    assert_eq!(ox[0].path.as_str(), "src/app.tsx");
    assert_eq!((ox[0].line, ox[0].column), (12, 5));
    assert_eq!(
        ox[0].remediation,
        "Oxlint help: Remove the debugger statement"
    );

    let oxlint_bare = r#"[{"message":"Fallback span","code":null,"filename":"src/b.ts","start_line":4,"start_column":2,"help":"   "}]"#;
    let ox_bare = parse_oxlint_json(oxlint_bare, Path::new("/repo"))?;
    assert_eq!((ox_bare[0].line, ox_bare[0].column), (4, 2));
    assert!(
        ox_bare[0]
            .remediation
            .contains("Resolve the `oxlint` violation")
    );
    Ok(())
}

#[test]
fn biome_and_tsc_pure_parsers_normalize_byte_offsets_and_continuations() -> TestRes {
    let win_root = Path::new("C:\\repo");
    let biome_json = r#"{
      "diagnostics": [
        {
          "category": "lint/suspicious/noExplicitAny",
          "severity": "error",
          "description": "Unexpected any.",
          "location": {
            "path": { "file": "C:\\repo\\src\\view.tsx" },
            "span": [19, 22],
            "sourceCode": "const ok = 1;\r\nlet bad: any = 2;\r\n"
          }
        },
        {
          "category": "lint/style/useConst",
          "severity": "warning",
          "description": "Ignored warning",
          "location": { "path": "src/view.tsx" }
        }
      ]
    }"#;
    let bm = parse_biome_json(biome_json, win_root)?;
    assert_eq!(bm.len(), 1);
    assert_eq!(bm[0].path.as_str(), "src/view.tsx");
    assert_eq!((bm[0].line, bm[0].column), (2, 5));
    assert_eq!(bm[0].native_code, "lint/suspicious/noExplicitAny");

    let tsc_raw = "C:\\repo\\src\\App (v2).tsx(14,7): error TS2322: Type 'string' is not assignable to type 'number'.\r\n  Type 'string' cannot be converted.\r\nFound 1 error.\r\n";
    let tsc = parse_tsc_output(tsc_raw, win_root)?;
    assert_eq!(tsc.len(), 1);
    assert_eq!(tsc[0].path.as_str(), "src/App (v2).tsx");
    assert_eq!((tsc[0].line, tsc[0].column), (14, 7));
    assert_eq!(tsc[0].native_code, "TS2322");
    assert_eq!(
        tsc[0].message,
        "Type 'string' is not assignable to type 'number'. Type 'string' cannot be converted."
    );
    Ok(())
}
