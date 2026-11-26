//! Property-based tests (`proptest`) for MariaDB, PostgreSQL/PL/pgSQL, Oracle PL/SQL,
//! lossless segmenters (`DELIMITER`, `/`, `$tag$`, `q'[...]'`), and `zpa-cli` JSON ingestion.

use std::path::Path;

use gf_core::RepoPath;
use gf_parsers::{parse_source, parse_zpa_generic_issues_json};
use proptest::prelude::*;

#[test]
fn db_proptest_sanity_checks_empty_zpa_and_minimal_sql() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        parse_zpa_generic_issues_json("", Path::new("/ws"))?.len(),
        0
    );
    let doc = parse_source(&RepoPath::new("sql/ok.sql")?, "SELECT 1;\n")?;
    assert_eq!(doc.lang_rule_findings.len(), 0);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn db_procedural_analyzers_never_panic_on_arbitrary_utf8(raw in "\\PC{0,180}") {
        for path_str in ["sql/fuzz.sql", "migrations/0001_fuzz.sql", "src/fuzz.pkb"] {
            if let Ok(path) = RepoPath::new(path_str) {
                let _ = parse_source(&path, &raw);
            }
        }
        let _ = parse_zpa_generic_issues_json(&raw, Path::new("/ws"));
    }

    #[test]
    fn db_line_numbers_covary_exactly_with_leading_comments(pad in 0u32..12u32) {
        let prefix = "-- comment\n".repeat(pad as usize);
        let plsql = format!(
            "{prefix}CREATE OR REPLACE PACKAGE p AS\n  PROCEDURE f;\nEND p;\n/\nCREATE OR REPLACE PACKAGE BODY p AS\n  PROCEDURE f IS\n  BEGIN\n    EXECUTE IMMEDIATE 'DELETE FROM t WHERE id = ' || v_id;\n  END f;\nEND p;\n/\n"
        );
        let p_path = RepoPath::new("src/p.pkb").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = parse_source(&p_path, &plsql).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let inj_line = doc
            .lang_rule_findings
            .iter()
            .find(|f| f.code() == "PLSQL-EXECUTE-IMMEDIATE-INJECTION")
            .map(|f| f.span.start_line());
        prop_assert_eq!(inj_line, Some(pad + 8));

        let pg = format!(
            "{prefix}-- garde-dialect: postgres\nCREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$\nBEGIN\n  SELECT a INTO v FROM t;\nEND;\n$$;\n"
        );
        let pg_path = RepoPath::new("sql/f.sql").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let pg_doc = parse_source(&pg_path, &pg).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let strict_line = pg_doc
            .lang_rule_findings
            .iter()
            .find(|f| f.code() == "PLPGSQL-MISSING-INTO-STRICT")
            .map(|f| f.span.start_line());
        prop_assert_eq!(strict_line, Some(pad + 4));
    }
}
