//! Tier 2 static ecosystem tool configuration auditor (`GF081`).
//!
//! Audits `pyproject.toml`, `ruff.toml`, `.ruff.toml`, and `tsconfig.json` to block
//! weakened type-checker or linter rules (`strict = false`, `ignore = ["ALL"]`).

use gf_core::{
    ExternalToolId, Language, RepoPath, Span, SyntaxErrorNode, ToolConfigFinding, UastDocument,
};

use super::jsonc::{
    CfgViolationSpec, audit_stryker_json, audit_tsconfig_json, line_span_containing,
    push_cfg_finding,
};
use crate::cst::line_metrics;
use crate::types::ParseError;

fn audit_mypy_override_flags(
    mypy: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    if mypy.get("ignore_errors").and_then(toml::Value::as_bool) == Some(true) {
        push_cfg_finding(
            out,
            CfgViolationSpec {
                tool: ExternalToolId::Mypy,
                key: "tool.mypy.ignore_errors",
                span: line_span_containing(source, "ignore_errors"),
                message: String::from("`ignore_errors = true` disables `mypy` type checking"),
                remediation: "Remove `ignore_errors = true` from `[tool.mypy]` and fix reported type errors.",
            },
        );
    }
    if mypy
        .get("disallow_untyped_defs")
        .and_then(toml::Value::as_bool)
        == Some(false)
    {
        push_cfg_finding(
            out,
            CfgViolationSpec {
                tool: ExternalToolId::Mypy,
                key: "tool.mypy.disallow_untyped_defs",
                span: line_span_containing(source, "disallow_untyped_defs"),
                message: String::from("`disallow_untyped_defs = false` weakens `mypy` strictness"),
                remediation: "Remove `disallow_untyped_defs = false` and type all function signatures.",
            },
        );
    }
}

fn audit_mypy_table(
    mypy: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let strict_ok = mypy
        .get("strict")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !strict_ok {
        push_cfg_finding(
            out,
            CfgViolationSpec {
                tool: ExternalToolId::Mypy,
                key: "tool.mypy.strict",
                span: line_span_containing(source, "[tool.mypy]"),
                message: String::from(
                    "`[tool.mypy]` must enforce `strict = true` in `pyproject.toml`",
                ),
                remediation: "Set `strict = true` inside `[tool.mypy]` in `pyproject.toml`.",
            },
        );
    }
    audit_mypy_override_flags(mypy, source, out);
}

fn has_blanket_ignore(tbl: &toml::map::Map<String, toml::Value>) -> bool {
    tbl.get("ignore")
        .and_then(toml::Value::as_array)
        .is_some_and(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .any(|s| s.eq_ignore_ascii_case("ALL") || s == "*")
        })
}

fn has_disabled_all_select(tbl: &toml::map::Map<String, toml::Value>) -> bool {
    let empty_select = tbl
        .get("select")
        .and_then(toml::Value::as_array)
        .is_some_and(Vec::is_empty);
    let has_extend = tbl
        .get("extend-select")
        .and_then(toml::Value::as_array)
        .is_some_and(|v| !v.is_empty());
    empty_select && !has_extend
}

fn audit_ruff_tables(
    root_tbl: &toml::map::Map<String, toml::Value>,
    key_prefix: &str,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let lint_tbl = root_tbl.get("lint").and_then(toml::Value::as_table);
    if has_blanket_ignore(root_tbl) || lint_tbl.is_some_and(has_blanket_ignore) {
        push_cfg_finding(
            out,
            CfgViolationSpec {
                tool: ExternalToolId::Ruff,
                key: &format!("{key_prefix}ignore"),
                span: line_span_containing(source, "ignore"),
                message: String::from(
                    "blanket `ignore = [\"ALL\"]` disables Ruff lint verification",
                ),
                remediation: "Remove `\"ALL\"` from `ignore` and remediate Ruff findings at their root cause.",
            },
        );
    }
    if has_disabled_all_select(root_tbl) || lint_tbl.is_some_and(has_disabled_all_select) {
        push_cfg_finding(
            out,
            CfgViolationSpec {
                tool: ExternalToolId::Ruff,
                key: &format!("{key_prefix}select"),
                span: line_span_containing(source, "select"),
                message: String::from("empty `select = []` disables all Ruff lint rules"),
                remediation: "Configure a non-empty rule set (e.g. `select = [\"E\", \"F\", \"I\", \"B\", \"UP\", \"SIM\"]`).",
            },
        );
    }
}

const BROAD_MUTANT_EXCLUDES: &[&str] = &["*", "**", "src/**", "crates/**"];

fn audit_mutants_exclude_globs(
    arr: &[toml::Value],
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    for pat in arr.iter().filter_map(toml::Value::as_str) {
        let norm = pat.trim().trim_start_matches("./");
        if BROAD_MUTANT_EXCLUDES.contains(&norm) {
            push_cfg_finding(
                out,
                CfgViolationSpec {
                    tool: ExternalToolId::CargoMutants,
                    key: "exclude_globs",
                    span: line_span_containing(source, "exclude_globs"),
                    message: format!(
                        "broad `exclude_globs` entry `{pat}` sabotages mutation scope"
                    ),
                    remediation: "Exclude only generated/template files, never entire `src/**` or `crates/**` trees.",
                },
            );
        }
    }
}

fn audit_mutants_toml(
    root_tbl: &toml::map::Map<String, toml::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    for key in ["examine_globs", "examine_re"] {
        if root_tbl
            .get(key)
            .and_then(toml::Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            push_cfg_finding(
                out,
                CfgViolationSpec {
                    tool: ExternalToolId::CargoMutants,
                    key,
                    span: line_span_containing(source, key),
                    message: format!("empty {key} = [] disables all Rust mutations"),
                    remediation: "Target real `Code` crates in `examine_globs` instead of an empty list.",
                },
            );
        }
    }
    if let Some(arr) = root_tbl
        .get("exclude_globs")
        .and_then(toml::Value::as_array)
    {
        audit_mutants_exclude_globs(arr, source, out);
    }
}

fn audit_toml_tool_config(file_name: &str, source: &str, doc: &mut UastDocument) {
    let parsed: toml::Value = match toml::from_str(source) {
        Ok(v) => v,
        Err(err) => {
            doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid TOML in `{file_name}`: {err}"),
                span: Span::FIRST,
            });
            return;
        }
    };
    let Some(root) = parsed.as_table() else {
        return;
    };
    if file_name == "mutants.toml" {
        audit_mutants_toml(root, source, &mut doc.tool_config_findings);
        return;
    }
    if file_name != "pyproject.toml" {
        audit_ruff_tables(root, "", source, &mut doc.tool_config_findings);
        return;
    }
    let Some(tool) = root.get("tool").and_then(toml::Value::as_table) else {
        return;
    };
    if let Some(mypy) = tool.get("mypy").and_then(toml::Value::as_table) {
        audit_mypy_table(mypy, source, &mut doc.tool_config_findings);
    }
    if let Some(ruff) = tool.get("ruff").and_then(toml::Value::as_table) {
        audit_ruff_tables(ruff, "tool.ruff.", source, &mut doc.tool_config_findings);
    }
}

/// Extracts Tier 2 ecosystem configuration findings (`GF081`) into a [`UastDocument`].
///
/// # Errors
///
/// Returns [`ParseError`] only if span initialization fails.
pub fn extract_tool_config(
    path: &RepoPath,
    lang: Language,
    source: &str,
) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), lang, total_lines, effective_lines)?;
    match path.file_name() {
        "tsconfig.json" => audit_tsconfig_json(source, &mut doc),
        "stryker.config.json" | "stryker.conf.json" => {
            audit_stryker_json(path.file_name(), source, &mut doc);
        }
        name => audit_toml_tool_config(name, source, &mut doc),
    }
    Ok(doc)
}
