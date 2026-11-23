//! Property-based tests for Markdown code-block HTML immunity (`GF063`) and
//! 6-domain path classification precedence (`classify_path`).

use gf_core::{Config, ContenusIssueKind, DomainKind, DomainLayoutConfig, RepoPath, classify_path};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;
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
fn markdown_html_inside_fenced_and_inline_code_never_triggers_gf063() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let path = RepoPath::new("content/lesson.md").map_err(|e| e.to_string())?;
    let mut tr = runner();

    let tag_strat = prop::sample::select(vec!["div", "span", "script", "style", "iframe", "img"]);
    let body_strat = "[a-zA-Z0-9 _-]{1,24}";

    tr.run(&(tag_strat, body_strat), |(tag, body)| {
        let md = format!(
            "# Lesson Title\n\n\
             Inline example `<{tag}>{body}</{tag}>` in prose.\n\n\
             ## Code Block\n\n\
             ```html\n\
             <{tag} class=\"demo\">{body}</{tag}>\n\
             ```\n\n\
             ~~~\n\
             <{tag}>{body}</{tag}>\n\
             ~~~\n"
        );
        let doc = parse_source(&path, &md).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let diags = evaluate_document(&doc, &cfg);
        prop_assert_eq!(diags, Vec::new());
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn markdown_raw_html_outside_code_always_triggers_gf063() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let path = RepoPath::new("content/impure.md").map_err(|e| e.to_string())?;
    let mut tr = runner();

    let tag_strat = prop::sample::select(vec!["div", "span", "script", "section", "table"]);
    let body_strat = "[a-zA-Z0-9]{1,16}";

    tr.run(&(tag_strat, body_strat), |(tag, body)| {
        let md = format!("# Pure Heading\n\n<{tag}>{body}</{tag}>\n");
        let doc = parse_source(&path, &md).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let has_raw_html = doc.contenus.as_ref().is_some_and(|c| {
            c.issues
                .iter()
                .any(|i| i.kind == ContenusIssueKind::RawHtml)
        });
        prop_assert!(has_raw_html);
        let diags = evaluate_document(&doc, &cfg);
        prop_assert!(diags.iter().any(|d| d.rule_id == "GF063"));
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn classify_path_prioritizes_specialized_subdomains_before_code() -> Result<(), String> {
    let layout = DomainLayoutConfig::strict_defaults();
    let mut tr = runner();
    let leaf_strat = "[a-z][a-z0-9_-]{1,12}";

    tr.run(&leaf_strat, |leaf| {
        let cases = [
            (format!("src/styles/{leaf}.css"), DomainKind::Forme),
            (format!("src/tokens/{leaf}.ts"), DomainKind::Forme),
            (format!("src/locales/{leaf}.json"), DomainKind::Texte),
            (format!("src/content/{leaf}.md"), DomainKind::Contenus),
            (format!("src/assets/{leaf}.svg"), DomainKind::Asset),
            (format!("scripts/{leaf}.sh"), DomainKind::Scripts),
            (format!("src/tests/{leaf}.ts"), DomainKind::Tests),
            (format!("src/domain/{leaf}.ts"), DomainKind::Code),
        ];
        for (raw, expected) in cases {
            let rp = RepoPath::new(&raw).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(
                classify_path(&rp, &layout),
                expected,
                "mismatch for {}",
                raw
            );
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}
