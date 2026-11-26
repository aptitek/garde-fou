//! Property-based tests (`proptest`) for Mermaid, `PlantUML`, ICU `MessageFormat`, `.po`, `.arb`, and `.ftl`.

use gf_core::RepoPath;
use gf_parsers::parse_source;
use proptest::prelude::*;

#[test]
fn diagrams_and_i18n_proptest_sanity() -> Result<(), Box<dyn std::error::Error>> {
    let mmd = parse_source(&RepoPath::new("docs/ok.mmd")?, "flowchart LR\n  A --> B\n")?;
    assert_eq!(mmd.lang_rule_findings.len(), 0);
    let puml = parse_source(
        &RepoPath::new("docs/ok.puml")?,
        "@startuml\nA -> B\n@enduml\n",
    )?;
    assert_eq!(puml.lang_rule_findings.len(), 0);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn diagrams_and_i18n_never_panic_on_arbitrary_utf8(raw in "\\PC{0,180}") {
        for p in [
            "docs/f.mmd",
            "docs/f.puml",
            "docs/f.md",
            "locales/en.json",
            "locales/en.po",
            "locales/en.ftl",
        ] {
            if let Ok(path) = RepoPath::new(p) {
                let _ = parse_source(&path, &raw);
            }
        }
    }

    #[test]
    fn fenced_mermaid_line_numbers_covary_with_leading_markdown_lines(pad in 0u32..12u32) {
        let prefix = "\n".repeat(pad as usize);
        let md = format!("{prefix}```mermaid\nflowchart LR\n  A --> end\n```\n");
        let path = RepoPath::new("docs/cov.md").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = parse_source(&path, &md).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let ln = doc
            .lang_rule_findings
            .iter()
            .find(|f| f.code() == "MERMAID-RESERVED-END-NODE")
            .map(|f| f.span.start_line());
        prop_assert_eq!(ln, Some(pad + 3));
    }
}
