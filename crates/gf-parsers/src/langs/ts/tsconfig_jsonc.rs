//! Pure `JSONC` configuration auditor for `tsconfig*.json` (with `"extends"`, `baseUrl`,
//! and `paths` extraction) and `biome.json` / `biome.jsonc` (`GF081`).

use std::collections::BTreeMap;

use gf_core::{
    Span, SyntaxErrorNode, ToolConfigFinding, ToolConfigViolation, TsconfigIssue,
    TsconfigStrictState, TsconfigSummary, UastDocument, is_known_strict_preset,
};

use crate::langs::jsonc::{line_span_containing, push_cfg_violation, strip_jsonc_to_json};

fn parse_extends_specifier(val: Option<&serde_json::Value>) -> Option<String> {
    let v = val?;
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        return (!trimmed.is_empty()).then(|| String::from(trimmed));
    }
    let arr = v.as_array()?;
    let strs: Vec<&str> = arr
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(&preset) = strs.iter().find(|&&s| is_known_strict_preset(s)) {
        return Some(String::from(preset));
    }
    strs.last().map(|&s| String::from(s))
}

fn extract_paths_table(
    opts: Option<&serde_json::Map<String, serde_json::Value>>,
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(paths_obj) = opts
        .and_then(|o| o.get("paths"))
        .and_then(serde_json::Value::as_object)
    else {
        return out;
    };
    for (pattern, targets_val) in paths_obj {
        let Some(arr) = targets_val.as_array() else {
            continue;
        };
        let targets: Vec<String> = arr
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if !targets.is_empty() {
            out.insert(pattern.clone(), targets);
        }
    }
    out
}

#[derive(Clone, Copy)]
struct TsconfigInput<'a> {
    file_name: &'a str,
    extends: Option<&'a str>,
    source: &'a str,
}

fn audit_tsconfig_sub_flags(
    input: TsconfigInput<'_>,
    opts: Option<&serde_json::Map<String, serde_json::Value>>,
    out: &mut Vec<ToolConfigFinding>,
) {
    for (sub_flag, issue) in [
        ("noImplicitAny", TsconfigIssue::NoImplicitAnyFalse),
        ("strictNullChecks", TsconfigIssue::StrictNullChecksFalse),
    ] {
        if opts
            .and_then(|o| o.get(sub_flag))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
        {
            push_cfg_violation(
                out,
                ToolConfigViolation::TsconfigNotStrict(issue),
                line_span_containing(input.source, sub_flag),
                format!(
                    "`\"{sub_flag}\": false` weakens TypeScript strict mode in `{}`",
                    input.file_name
                ),
            );
        }
    }
}

fn audit_tsconfig_compiler_options(
    input: TsconfigInput<'_>,
    opts: Option<&serde_json::Map<String, serde_json::Value>>,
    out: &mut Vec<ToolConfigFinding>,
) -> TsconfigStrictState {
    let file_name = input.file_name;
    let source = input.source;
    let strict_state = match opts
        .and_then(|o| o.get("strict"))
        .and_then(serde_json::Value::as_bool)
    {
        Some(true) => TsconfigStrictState::ExplicitTrue,
        Some(false) => {
            push_cfg_violation(
                out,
                ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict),
                line_span_containing(source, "strict"),
                format!("`{file_name}` explicitly sets `\"strict\": false` in `compilerOptions`"),
            );
            TsconfigStrictState::ExplicitFalse
        }
        None => {
            if input.extends.is_none() {
                push_cfg_violation(
                    out,
                    ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict),
                    line_span_containing(source, "compilerOptions"),
                    format!(
                        "`{file_name}` must enforce `\"strict\": true` inside `compilerOptions`"
                    ),
                );
            }
            TsconfigStrictState::Omitted
        }
    };
    audit_tsconfig_sub_flags(input, opts, out);
    strict_state
}

/// Audits `tsconfig.json` / `tsconfig.*.json` (JSONC-aware) and populates `doc.tsconfig`.
pub fn audit_tsconfig_jsonc(file_name: &str, source: &str, doc: &mut UastDocument) {
    let cleaned = strip_jsonc_to_json(source);
    let parsed: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(err) => {
            doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid JSON/JSONC in `{file_name}`: {err}"),
                span: Span::FIRST,
            });
            return;
        }
    };
    let extends = parse_extends_specifier(parsed.get("extends"));
    let opts = parsed
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object);
    let strict = audit_tsconfig_compiler_options(
        TsconfigInput {
            file_name,
            extends: extends.as_deref(),
            source,
        },
        opts,
        &mut doc.tool_config_findings,
    );
    let base_url = opts
        .and_then(|o| o.get("baseUrl"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let paths = extract_paths_table(opts);
    let strict_span = if extends.is_some() {
        line_span_containing(source, "extends")
    } else {
        Span::FIRST
    };
    doc.tsconfig = Some(TsconfigSummary {
        extends,
        strict,
        base_url,
        paths,
        strict_span,
    });
}

/// Audits `biome.json` / `biome.jsonc` for disabled linter (`"linter": { "enabled": false }`).
pub fn audit_biome_jsonc(file_name: &str, source: &str, doc: &mut UastDocument) {
    let cleaned = strip_jsonc_to_json(source);
    let parsed: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(err) => {
            doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid JSON/JSONC in `{file_name}`: {err}"),
                span: Span::FIRST,
            });
            return;
        }
    };
    if parsed
        .get("linter")
        .and_then(|l| l.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        push_cfg_violation(
            &mut doc.tool_config_findings,
            ToolConfigViolation::BiomeLinterDisabled,
            line_span_containing(source, "enabled"),
            format!("`\"linter.enabled\": false` in `{file_name}` disables Biome verification"),
        );
    }
}
