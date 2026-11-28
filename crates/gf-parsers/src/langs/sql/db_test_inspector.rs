//! Database TDD test suite inspector for `utPLSQL` (`--%test` + `ut.expect`) and `pgTAP`
//! (`SELECT plan(...)`, `SELECT ok/is/throws_ok/results_eq(...)`) feeding `GF076`.

use gf_core::{RepoPath, Span, TestCaseSummary};

const PGTAP_ASSERTIONS: &[&str] = &[
    "ok(",
    "is(",
    "isnt(",
    "throws_ok(",
    "lives_ok(",
    "results_eq(",
    "set_eq(",
    "bag_eq(",
    "has_table(",
    "has_column(",
    "has_policy(",
    "policies_are(",
    "is_empty(",
];

fn is_db_test_path(path: &RepoPath) -> bool {
    let raw = path.as_str();
    let name = path.file_name().to_ascii_lowercase();
    raw.starts_with("tests/")
        || raw.starts_with("test/")
        || raw.contains("/tests/")
        || raw.contains("/test/")
        || name.starts_with("test_")
        || name.contains("_test.")
}

fn extract_proc_name(low: &str) -> Option<String> {
    let rest = low.strip_prefix("procedure ")?;
    let id: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!id.is_empty()).then_some(id)
}

fn is_utplsql_block_stop(low: &str, has_body: bool) -> bool {
    low.starts_with("--%test") || (low.starts_with("procedure ") && low.ends_with(';') && !has_body)
}

fn parse_utplsql_block(lines: &[&str], start_idx: usize) -> (usize, Option<TestCaseSummary>) {
    let start_ln = u32::try_from(start_idx + 1).unwrap_or(u32::MAX);
    let mut name = format!("utplsql_test_{start_ln}");
    let mut assertions = 0u32;
    let (mut end_ln, mut has_body) = (start_ln, false);
    let mut j = start_idx + 1;
    while j < lines.len() {
        let low = lines[j].trim().to_ascii_lowercase();
        if is_utplsql_block_stop(&low, has_body) {
            break;
        }
        has_body |= low == "begin" || low.starts_with("begin ");
        if let Some(id) = extract_proc_name(&low) {
            name = id;
        }
        assertions += u32::from(low.contains("ut.expect(") && low.contains(".to_"));
        end_ln = u32::try_from(j + 1).unwrap_or(u32::MAX);
        if low.starts_with("end ") || low == "end;" {
            break;
        }
        j += 1;
    }
    let summary = has_body.then(|| TestCaseSummary {
        name,
        span: Span::new(start_ln, end_ln.max(start_ln)).unwrap_or(Span::FIRST),
        assertion_count: assertions,
        has_tautology: false,
        has_weak_only_assertion: false,
    });
    (j.max(start_idx + 1), summary)
}

fn extract_utplsql_tests(source: &str, out: &mut Vec<TestCaseSummary>) {
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        if !lines[i].trim().to_ascii_lowercase().starts_with("--%test") {
            i += 1;
            continue;
        }
        let (next, summary) = parse_utplsql_block(&lines, i);
        if let Some(tc) = summary {
            out.push(tc);
        }
        i = next;
    }
}

fn extract_pgtap_tests(source: &str, out: &mut Vec<TestCaseSummary>) {
    let low = source.to_ascii_lowercase();
    if !low.contains("select plan(") && !low.contains("select no_plan(") {
        return;
    }
    let mut assertions = 0u32;
    let mut plan_line = 1u32;
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let l = line.split("--").next().unwrap_or(line).to_ascii_lowercase();
        if l.contains("select plan(") || l.contains("select no_plan(") {
            plan_line = ln;
        }
        if PGTAP_ASSERTIONS.iter().any(|a| l.contains(a)) {
            assertions = assertions.saturating_add(1);
        }
    }
    let end_ln = u32::try_from(source.lines().count().max(1)).unwrap_or(1);
    out.push(TestCaseSummary {
        name: String::from("pgtap_suite"),
        span: Span::new(plan_line, end_ln.max(plan_line)).unwrap_or(Span::FIRST),
        assertion_count: assertions,
        has_tautology: false,
        has_weak_only_assertion: false,
    });
}

/// Extracts `utPLSQL` and `pgTAP` [`TestCaseSummary`] records for `GF076` test-quality enforcement.
pub fn extract_db_test_cases(path: &RepoPath, source: &str, out: &mut Vec<TestCaseSummary>) {
    if !is_db_test_path(path) {
        return;
    }
    extract_utplsql_tests(source, out);
    extract_pgtap_tests(source, out);
}
