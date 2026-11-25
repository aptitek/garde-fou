//! Tier 2 static configuration auditor (`GF081`) for `rumdl.toml` and `taplo.toml`.

use gf_core::{
    FormatConfigIssue, MarkdownConfigIssue, RepoPath, Span, TaploConfigIssue, ToolConfigViolation,
    UastDocument,
};
use toml_edit::{ImDocument, Item, Value};

use super::format_tool_config::CRITICAL_MD_RULES;
use super::jsonc::push_cfg_violation;
use super::toml_lang::{analyze_toml_into_document, span_from_byte_offset};

const BLANKET_TAPLO_EXCLUDES: &[&str] = &["**/*.toml", "*.toml", "*", "**", "**/*"];

fn check_rumdl_disable_array(source: &str, item: &Item, doc: &mut UastDocument) {
    let Some(arr) = item.as_array() else {
        return;
    };
    for val in arr {
        if let Some(s) = val.as_str()
            && CRITICAL_MD_RULES.contains(&s.trim().to_ascii_lowercase().as_str())
        {
            let sp = val
                .span()
                .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
            push_cfg_violation(
                &mut doc.tool_config_findings,
                ToolConfigViolation::Formats(FormatConfigIssue::Markdown(
                    MarkdownConfigIssue::CriticalRuleDisabled,
                )),
                sp,
                format!("`rumdl.toml` disables mandatory Markdown rule `{s}`"),
            );
        }
    }
}

/// Audits `rumdl.toml` / `.rumdl.toml` (`GF081` + Tier 1 TOML syntax/structure).
pub fn audit_rumdl_toml(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    analyze_toml_into_document(path, source, doc);
    let Ok(im_doc) = ImDocument::parse(source) else {
        return;
    };
    let root = im_doc.as_table();
    if let Some(dis) = root.get("disable") {
        check_rumdl_disable_array(source, dis, doc);
    }
    if let Some(glob_dis) = root
        .get("global")
        .and_then(Item::as_table_like)
        .and_then(|g| g.get("disable"))
    {
        check_rumdl_disable_array(source, glob_dis, doc);
    }
}

/// Audits `taplo.toml` / `.taplo.toml` (`GF081` + Tier 1 TOML syntax/structure).
pub fn audit_taplo_toml(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    analyze_toml_into_document(path, source, doc);
    let Ok(im_doc) = ImDocument::parse(source) else {
        return;
    };
    let Some(arr) = im_doc.as_table().get("exclude").and_then(Item::as_array) else {
        return;
    };
    for val in arr {
        if let Value::String(s) = val {
            let norm = s.value().trim().trim_start_matches("./");
            if BLANKET_TAPLO_EXCLUDES.contains(&norm) {
                let sp = s
                    .span()
                    .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
                push_cfg_violation(
                    &mut doc.tool_config_findings,
                    ToolConfigViolation::Formats(FormatConfigIssue::Taplo(
                        TaploConfigIssue::WildcardExcluded,
                    )),
                    sp,
                    format!(
                        "`taplo.toml` blanket `exclude` pattern `{}` disables TOML verification",
                        s.value()
                    ),
                );
            }
        }
    }
}
