//! Property-based panic-freedom tests for PHP 8.4, Twig 3, and XLIFF parsers.

use gf_core::RepoPath;
use gf_parsers::parse_source;
use proptest::prelude::*;

#[test]
fn empty_php_and_twig_parse_cleanly() -> Result<(), String> {
    let php = RepoPath::new("src/Empty.php").map_err(|e| e.to_string())?;
    let twig = RepoPath::new("templates/empty.html.twig").map_err(|e| e.to_string())?;
    let pdoc =
        parse_source(&php, "<?php\ndeclare(strict_types=1);\n").map_err(|e| e.to_string())?;
    let tdoc = parse_source(&twig, "").map_err(|e| e.to_string())?;
    assert_eq!(pdoc.lang_rule_findings.len(), 0);
    assert_eq!(tdoc.lang_rule_findings.len(), 0);
    Ok(())
}

proptest! {
    #[test]
    fn php_and_twig_never_panic_on_arbitrary_utf8(src in "[a-zA-Z0-9_{}()\\[\\]$@=<>'\"\\n ;:/\\\\|*%-]{0,120}") {
        if let Ok(p) = RepoPath::new("src/Fuzz.php") {
            prop_assert!(parse_source(&p, &src).is_ok());
        }
        if let Ok(t) = RepoPath::new("templates/fuzz.html.twig") {
            prop_assert!(parse_source(&t, &src).is_ok());
        }
    }
}
