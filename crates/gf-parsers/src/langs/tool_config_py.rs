//! Pure Tier 2 auditor for Python `[tool.mypy]` and `[tool.ruff]` / `ruff.toml` (`GF081`).

use gf_core::{MypyConfigIssue, RuffConfigIssue, Span, ToolConfigFinding, ToolConfigViolation};

use super::jsonc::push_cfg_violation;

pub(super) fn section_key_span(source: &str, section_header: &str, key_needle: &str) -> Span {
    let lines: Vec<&str> = source.lines().collect();
    let start_idx = lines
        .iter()
        .position(|l| !l.trim_start().starts_with('#') && l.contains(section_header))
        .unwrap_or(0);
    for (offset, line) in lines.iter().skip(start_idx).enumerate() {
        let trimmed = line.trim_start();
        if offset > 0 && trimmed.starts_with('[') {
            break;
        }
        if !trimmed.starts_with('#') && line.contains(key_needle) {
            let ln = u32::try_from(start_idx + offset)
                .unwrap_or(0)
                .saturating_add(1);
            return Span::new(ln, ln).unwrap_or(Span::FIRST);
        }
    }
    let fallback = u32::try_from(start_idx).unwrap_or(0).saturating_add(1);
    Span::new(fallback, fallback).unwrap_or(Span::FIRST)
}

fn is_test_only_mypy_module(pat: &str) -> bool {
    let p = pat.trim();
    p == "tests"
        || p.starts_with("tests.")
        || p == "test"
        || p.starts_with("test_")
        || p == "conftest"
        || p.rsplit('.')
            .next()
            .is_some_and(|seg| matches!(seg, "tests" | "_tests"))
}

fn override_targets_production(ov: &toml::map::Map<String, toml::Value>) -> bool {
    match ov.get("module") {
        Some(toml::Value::String(s)) => !is_test_only_mypy_module(s),
        Some(toml::Value::Array(arr)) => {
            arr.is_empty()
                || arr
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .any(|s| !is_test_only_mypy_module(s))
        }
        _ => true,
    }
}

fn audit_mypy_overrides(
    mypy: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let Some(overrides) = mypy.get("overrides").and_then(toml::Value::as_array) else {
        return;
    };
    for ov in overrides.iter().filter_map(toml::Value::as_table) {
        if !override_targets_production(ov) {
            continue;
        }
        let weakened = ov.get("ignore_errors").and_then(toml::Value::as_bool) == Some(true)
            || ov.get("strict").and_then(toml::Value::as_bool) == Some(false)
            || ov
                .get("disallow_untyped_defs")
                .and_then(toml::Value::as_bool)
                == Some(false);
        if weakened {
            push_cfg_violation(
                out,
                ToolConfigViolation::MypyNotStrict(MypyConfigIssue::ProductionOverrideWeakened),
                section_key_span(source, "[[tool.mypy.overrides]]", "ignore_errors"),
                String::from(
                    "`[[tool.mypy.overrides]]` disables strict type checking on production modules",
                ),
            );
        }
    }
}

pub(super) fn audit_mypy_table(
    mypy: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let strict_ok = mypy
        .get("strict")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !strict_ok {
        let needle = if mypy.contains_key("strict") {
            "strict"
        } else {
            "[tool.mypy]"
        };
        push_cfg_violation(
            out,
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::MissingOrFalseStrict),
            section_key_span(source, "[tool.mypy]", needle),
            String::from("`[tool.mypy]` must enforce `strict = true` in `pyproject.toml`"),
        );
    }
    if mypy.get("ignore_errors").and_then(toml::Value::as_bool) == Some(true) {
        push_cfg_violation(
            out,
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::IgnoreErrors),
            section_key_span(source, "[tool.mypy]", "ignore_errors"),
            String::from("`ignore_errors = true` disables `mypy` type checking"),
        );
    }
    for (flag, issue) in [
        (
            "disallow_untyped_defs",
            MypyConfigIssue::DisallowUntypedDefsFalse,
        ),
        ("warn_return_any", MypyConfigIssue::WarnReturnAnyFalse),
    ] {
        if mypy.get(flag).and_then(toml::Value::as_bool) == Some(false) {
            push_cfg_violation(
                out,
                ToolConfigViolation::MypyNotStrict(issue),
                section_key_span(source, "[tool.mypy]", flag),
                format!("`{flag} = false` weakens `mypy` strictness"),
            );
        }
    }
    audit_mypy_overrides(mypy, source, out);
}

fn array_has_blanket_wildcard(tbl: &toml::map::Map<String, toml::Value>, keys: &[&str]) -> bool {
    keys.iter().any(|&k| {
        tbl.get(k)
            .and_then(toml::Value::as_array)
            .is_some_and(|arr| {
                arr.iter()
                    .filter_map(toml::Value::as_str)
                    .any(|s| s.eq_ignore_ascii_case("ALL") || s == "*")
            })
    })
}

fn tables_have_blanket(
    root_tbl: &toml::map::Map<String, toml::Value>,
    lint_tbl: Option<&toml::map::Map<String, toml::Value>>,
    keys: &[&str],
) -> bool {
    array_has_blanket_wildcard(root_tbl, keys)
        || lint_tbl.is_some_and(|l| array_has_blanket_wildcard(l, keys))
}

fn has_disabled_all_select(tbl: &toml::map::Map<String, toml::Value>) -> bool {
    let empty_select = tbl
        .get("select")
        .and_then(toml::Value::as_array)
        .is_some_and(Vec::is_empty);
    let has_extend = ["extend-select", "extend_select"].iter().any(|&k| {
        tbl.get(k)
            .and_then(toml::Value::as_array)
            .is_some_and(|v| !v.is_empty())
    });
    empty_select && !has_extend
}

const fn choose_ruff_issue(
    in_pyproject: bool,
    pyproj: RuffConfigIssue,
    standalone: RuffConfigIssue,
) -> RuffConfigIssue {
    if in_pyproject { pyproj } else { standalone }
}

pub(super) fn audit_ruff_tables(
    root_tbl: &toml::map::Map<String, toml::Value>,
    in_pyproject: bool,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let lint_tbl = root_tbl.get("lint").and_then(toml::Value::as_table);
    let hdr = if in_pyproject { "[tool.ruff" } else { "" };
    if tables_have_blanket(root_tbl, lint_tbl, &["ignore"]) {
        let issue = choose_ruff_issue(
            in_pyproject,
            RuffConfigIssue::PyprojectBlanketIgnore,
            RuffConfigIssue::StandaloneBlanketIgnore,
        );
        push_cfg_violation(
            out,
            ToolConfigViolation::RuffWeakened(issue),
            section_key_span(source, hdr, "ignore"),
            String::from("blanket `ignore = [\"ALL\"]` disables Ruff lint verification"),
        );
    }
    if tables_have_blanket(root_tbl, lint_tbl, &["extend-ignore", "extend_ignore"]) {
        let issue = choose_ruff_issue(
            in_pyproject,
            RuffConfigIssue::PyprojectBlanketExtendIgnore,
            RuffConfigIssue::StandaloneBlanketExtendIgnore,
        );
        push_cfg_violation(
            out,
            ToolConfigViolation::RuffWeakened(issue),
            section_key_span(source, hdr, "extend"),
            String::from("blanket `extend-ignore = [\"ALL\"]` disables Ruff lint verification"),
        );
    }
    if has_disabled_all_select(root_tbl) || lint_tbl.is_some_and(has_disabled_all_select) {
        let issue = choose_ruff_issue(
            in_pyproject,
            RuffConfigIssue::PyprojectEmptySelect,
            RuffConfigIssue::StandaloneEmptySelect,
        );
        push_cfg_violation(
            out,
            ToolConfigViolation::RuffWeakened(issue),
            section_key_span(source, hdr, "select"),
            String::from("empty `select = []` disables all Ruff lint rules"),
        );
    }
}
