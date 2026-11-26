//! Property-based panic-freedom tests for Minecraft `.mcfunction` and manifest parsers.

use gf_core::RepoPath;
use gf_parsers::parse_source;
use proptest::prelude::*;

#[test]
fn empty_mcfunction_parses_with_zero_findings() -> Result<(), String> {
    let p = RepoPath::new("data/demo/function/empty.mcfunction").map_err(|e| e.to_string())?;
    let doc = parse_source(&p, "# clean comment\n").map_err(|e| e.to_string())?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    Ok(())
}

proptest! {
    #[test]
    fn mcfunction_parser_never_panics_on_arbitrary_input(src in "[a-zA-Z0-9_@$()\\[\\]/#:\\n =.,-]{0,120}") {
        if let Ok(p) = RepoPath::new("data/demo/function/fuzz.mcfunction") {
            prop_assert!(parse_source(&p, &src).is_ok());
        }
    }
}
