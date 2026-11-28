//! Tier 2 static audit and binding extractor for `wrangler.toml`, `wrangler.json`,
//! and `wrangler.jsonc` (`CF-WRANGLER-CONFIG` & `CF-BINDING-COHERENCE` — `GF080`/`GF081`).

use std::collections::BTreeSet;

use gf_core::{
    RepoPath, Span, SyntaxErrorNode, ToolConfigFinding, ToolConfigViolation, UastDocument,
    WebEdgeConfigIssue, WebEdgeSummary, WranglerSummary,
};
use serde_json::Value;

use crate::langs::jsonc::{line_span_containing, strip_jsonc_to_json};
use crate::langs::toml_lang::span_from_byte_offset;

#[rustfmt::skip]
const SENSITIVE_VAR_MARKERS: &[&str] = &[
    "SECRET", "PASSWORD", "PASSWD", "TOKEN", "API_KEY", "PRIVATE_KEY",
];

fn is_valid_yyyy_mm_dd(raw: &str) -> bool {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let (Ok(yr), Ok(mo)) = (trimmed[0..4].parse::<u32>(), trimmed[5..7].parse::<u32>()) else {
        return false;
    };
    let Ok(day) = trimmed[8..10].parse::<u32>() else {
        return false;
    };
    (2020..=2100).contains(&yr) && (1..=12).contains(&mo) && (1..=31).contains(&day)
}

fn push_wrangler_finding(
    out: &mut Vec<ToolConfigFinding>,
    issue: WebEdgeConfigIssue,
    span: Span,
    message: String,
) {
    out.push(ToolConfigFinding {
        violation: ToolConfigViolation::WebEdge(issue),
        span,
        message,
    });
}

#[derive(Clone, Copy)]
struct BindingScanSpec<'a> {
    binding_field: &'a str,
    is_d1_or_r2: bool,
    source: &'a str,
}

fn record_binding_entry(
    obj: &serde_json::Map<String, Value>,
    spec: BindingScanSpec<'_>,
    summary: &mut WranglerSummary,
    out: &mut Vec<ToolConfigFinding>,
) {
    if let Some(trimmed) = obj
        .get(spec.binding_field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        summary.declared_bindings.insert(String::from(trimmed));
        if spec.is_d1_or_r2 {
            let sp = line_span_containing(spec.source, trimmed);
            summary.d1_r2_bindings.push((String::from(trimmed), sp));
        }
    }
    if obj.get("remote").and_then(Value::as_bool) == Some(true) {
        push_wrangler_finding(
            out,
            WebEdgeConfigIssue::WranglerUnguardedRemoteBinding,
            line_span_containing(spec.source, "remote"),
            String::from(
                "[CF-WRANGLER-CONFIG] binding enables `remote = true` by default in `wrangler.*`",
            ),
        );
    }
}

fn check_binding_array(
    arr: &[Value],
    spec: BindingScanSpec<'_>,
    summary: &mut WranglerSummary,
    out: &mut Vec<ToolConfigFinding>,
) {
    for entry in arr {
        if let Some(obj) = entry.as_object() {
            record_binding_entry(obj, spec, summary, out);
        }
    }
}

#[rustfmt::skip]
fn collect_queues_and_do_bindings(root_obj: &serde_json::Map<String, Value>, source: &str, summary: &mut WranglerSummary, out: &mut Vec<ToolConfigFinding>) {
    if let Some(queues) = root_obj.get("queues").and_then(Value::as_object) {
        let spec = BindingScanSpec { binding_field: "binding", is_d1_or_r2: false, source };
        for sub in ["producers", "consumers"] {
            if let Some(arr) = queues.get(sub).and_then(Value::as_array) {
                check_binding_array(arr, spec, summary, out);
            }
        }
    }
    if let Some(do_arr) = root_obj
        .get("durable_objects")
        .and_then(|d| d.get("bindings"))
        .and_then(Value::as_array)
    {
        let spec = BindingScanSpec { binding_field: "name", is_d1_or_r2: false, source };
        check_binding_array(do_arr, spec, summary, out);
    }
    if let Some(ai_b) = root_obj
        .get("ai")
        .and_then(|a| a.get("binding"))
        .and_then(Value::as_str)
    {
        summary.declared_bindings.insert(String::from(ai_b.trim()));
    }
}

fn collect_all_wrangler_bindings(
    root_obj: &serde_json::Map<String, Value>,
    source: &str,
    summary: &mut WranglerSummary,
    out: &mut Vec<ToolConfigFinding>,
) {
    for (section, is_d1_or_r2) in [
        ("d1_databases", true),
        ("r2_buckets", true),
        ("kv_namespaces", false),
        ("services", false),
        ("vectorize", false),
        ("hyperdrive", false),
    ] {
        let Some(arr) = root_obj.get(section).and_then(Value::as_array) else {
            continue;
        };
        if section == "d1_databases" && !arr.is_empty() {
            summary.has_d1 = true;
        }
        let spec = BindingScanSpec {
            binding_field: "binding",
            is_d1_or_r2,
            source,
        };
        check_binding_array(arr, spec, summary, out);
    }
    collect_queues_and_do_bindings(root_obj, source, summary, out);
}

fn check_wrangler_vars(
    root_obj: &serde_json::Map<String, Value>,
    source: &str,
    summary: &mut WranglerSummary,
    out: &mut Vec<ToolConfigFinding>,
) {
    let Some(vars) = root_obj.get("vars").and_then(Value::as_object) else {
        return;
    };
    for (k, v) in vars {
        summary.declared_bindings.insert(k.clone());
        let upper = k.to_ascii_uppercase();
        let val_str = v.as_str().unwrap_or("");
        if SENSITIVE_VAR_MARKERS.iter().any(|m| upper.contains(m)) && !val_str.trim().is_empty() {
            push_wrangler_finding(
                out,
                WebEdgeConfigIssue::WranglerPlaintextSecretInVars,
                line_span_containing(source, k),
                format!(
                    "[CF-WRANGLER-CONFIG] plaintext secret `{k}` in `wrangler.*` `[vars]`; use `wrangler secret put`"
                ),
            );
        }
    }
}

#[rustfmt::skip]
fn audit_wrangler_root(root_obj: &serde_json::Map<String, Value>, source: &str, doc: &mut UastDocument) {
    let mut summary = WranglerSummary {
        has_nodejs_compat: false,
        has_d1: false,
        declared_bindings: BTreeSet::new(),
        d1_r2_bindings: Vec::new(),
        span: Span::FIRST,
    };
    let compat_ok = root_obj
        .get("compatibility_date")
        .and_then(Value::as_str)
        .is_some_and(is_valid_yyyy_mm_dd);
    if !compat_ok {
        push_wrangler_finding(
            &mut doc.tool_config_findings,
            WebEdgeConfigIssue::WranglerInvalidCompatibilityDate,
            line_span_containing(source, "compatibility_date"),
            String::from("[CF-WRANGLER-CONFIG] `wrangler.*` must declare a valid `compatibility_date` (`YYYY-MM-DD`)"),
        );
    }
    if let Some(flags) = root_obj.get("compatibility_flags").and_then(Value::as_array) {
        summary.has_nodejs_compat = flags
            .iter()
            .filter_map(Value::as_str)
            .any(|f| matches!(f.trim(), "nodejs_compat" | "nodejs_compat_v2"));
    }
    check_wrangler_vars(root_obj, source, &mut summary, &mut doc.tool_config_findings);
    collect_all_wrangler_bindings(root_obj, source, &mut summary, &mut doc.tool_config_findings);
    doc.web_edge.get_or_insert_with(WebEdgeSummary::default).wrangler = Some(summary);
}

/// Audits `wrangler.toml`, `wrangler.json`, or `wrangler.jsonc`.
#[rustfmt::skip]
pub fn audit_wrangler_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let is_toml = std::path::Path::new(path.file_name())
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
    let val: Value = if is_toml {
        match toml::from_str(source) {
            Ok(v) => v,
            Err(err) => {
                let sp = err.span().map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
                doc.syntax_errors.push(SyntaxErrorNode { span: sp, message: err.message().to_string() });
                return;
            }
        }
    } else {
        let stripped = strip_jsonc_to_json(source);
        match serde_json::from_str(&stripped) {
            Ok(v) => v,
            Err(err) => {
                let ln = u32::try_from(err.line().max(1)).unwrap_or(1);
                doc.syntax_errors.push(SyntaxErrorNode {
                    span: Span::new(ln, ln).unwrap_or(Span::FIRST),
                    message: err.to_string(),
                });
                return;
            }
        }
    };
    if let Some(root_obj) = val.as_object() {
        audit_wrangler_root(root_obj, source, doc);
    }
}
