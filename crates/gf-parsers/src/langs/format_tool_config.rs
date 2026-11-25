//! Tier 2 static configuration auditor (`GF081`) for `.stylelintrc*`, `.markdownlint*`,
//! and `svgo.config.{js,mjs,cjs}`.

use gf_core::{
    FormatConfigIssue, MarkdownConfigIssue, Span, StylelintConfigIssue, SvgoConfigIssue,
    ToolConfigViolation, UastDocument,
};

use super::jsonc::{push_cfg_violation, strip_jsonc_to_json};
use super::yaml::{
    anchors_merge::ResolvedYaml, anchors_merge::resolve_yaml_document,
    events_tree::parse_yaml_stream,
};

const CRITICAL_STYLELINT_RULES: &[&str] = &[
    "color-no-hex",
    "declaration-no-important",
    "selector-max-id",
];

pub(super) const CRITICAL_MD_RULES: &[&str] = &[
    "md001",
    "heading-increment",
    "md033",
    "no-inline-html",
    "all",
    "*",
];

fn is_yaml_scalar_disabled(val: &str) -> bool {
    matches!(
        val.trim().to_ascii_lowercase().as_str(),
        "null" | "~" | "false" | "off" | "0" | ""
    )
}

fn is_yaml_rule_off(node: &ResolvedYaml) -> bool {
    match node {
        ResolvedYaml::Scalar(s) => is_yaml_scalar_disabled(&s.value),
        ResolvedYaml::Sequence { items, .. } => items
            .first()
            .and_then(ResolvedYaml::as_scalar)
            .is_some_and(|s| is_yaml_scalar_disabled(&s.value)),
        ResolvedYaml::Mapping { .. } => false,
    }
}

fn parse_jsonc_or_yaml_config(
    file_name: &str,
    source: &str,
    doc: &mut UastDocument,
) -> Option<ResolvedYaml> {
    let cleaned;
    let input = if file_name.to_ascii_lowercase().contains(".json")
        || source.trim_start().starts_with('{')
    {
        cleaned = strip_jsonc_to_json(source);
        cleaned.as_str()
    } else {
        source
    };
    let stream = parse_yaml_stream(input);
    doc.syntax_errors.extend(stream.syntax_errors);
    let first = stream.docs.first()?;
    resolve_yaml_document(first, &mut doc.lang_rule_findings)
}

fn push_fmt_cfg(doc: &mut UastDocument, issue: FormatConfigIssue, span: Span, msg: String) {
    push_cfg_violation(
        &mut doc.tool_config_findings,
        ToolConfigViolation::Formats(issue),
        span,
        msg,
    );
}

/// Audits `.stylelintrc`, `.stylelintrc.json`, and `.stylelintrc.yml`/`.yaml` (`GF081`).
pub fn audit_stylelint_config(file_name: &str, source: &str, doc: &mut UastDocument) {
    let Some(root) = parse_jsonc_or_yaml_config(file_name, source, doc) else {
        return;
    };
    let Some(ResolvedYaml::Mapping { entries, .. }) = root.get("rules") else {
        return;
    };
    for (k, v) in entries {
        if CRITICAL_STYLELINT_RULES.contains(&k.value.trim().to_ascii_lowercase().as_str())
            && is_yaml_rule_off(v)
        {
            push_fmt_cfg(
                doc,
                FormatConfigIssue::Stylelint(StylelintConfigIssue::CriticalRuleDisabled),
                k.span,
                format!(
                    "`.stylelintrc` disables critical token/specificity rule `{}`",
                    k.value
                ),
            );
        }
    }
}

/// Audits `.markdownlint.{json,jsonc,yml,yaml}` (`GF081`).
pub fn audit_markdownlint_config(file_name: &str, source: &str, doc: &mut UastDocument) {
    let Some(ResolvedYaml::Mapping { entries, .. }) =
        parse_jsonc_or_yaml_config(file_name, source, doc)
    else {
        return;
    };
    for (k, v) in &entries {
        let lower = k.value.trim().to_ascii_lowercase();
        if lower == "default"
            && v.as_scalar()
                .is_some_and(|s| s.value.trim().eq_ignore_ascii_case("false"))
        {
            push_fmt_cfg(
                doc,
                FormatConfigIssue::Markdown(MarkdownConfigIssue::DefaultDisabled),
                k.span,
                String::from("`.markdownlint` sets `\"default\": false`, disabling baseline rules"),
            );
        } else if CRITICAL_MD_RULES.contains(&lower.as_str()) && is_yaml_rule_off(v) {
            push_fmt_cfg(
                doc,
                FormatConfigIssue::Markdown(MarkdownConfigIssue::CriticalRuleDisabled),
                k.span,
                format!("`.markdownlint` disables mandatory rule `{}`", k.value),
            );
        }
    }
}

/// Audits `svgo.config.{js,mjs,cjs}` for `removeScripts: false` (`GF081`).
pub fn audit_svgo_config(source: &str, doc: &mut UastDocument) {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .is_err()
    {
        return;
    }
    let Some(tree) = parser.parse(source, None) else {
        return;
    };
    scan_svgo_pairs(tree.root_node(), source, doc);
}

fn scan_svgo_pairs(node: tree_sitter::Node<'_>, source: &str, doc: &mut UastDocument) {
    if node.kind() == "pair"
        && let (Some(k), Some(v)) = (
            node.child_by_field_name("key"),
            node.child_by_field_name("value"),
        )
    {
        let key_txt = source
            .get(k.byte_range())
            .unwrap_or("")
            .trim()
            .trim_matches(['"', '\'']);
        let val_txt = source.get(v.byte_range()).unwrap_or("").trim();
        if key_txt == "removeScripts" && val_txt == "false" {
            let line = u32::try_from(k.start_position().row).unwrap_or(0) + 1;
            let sp = Span::new(line, line).unwrap_or(Span::FIRST);
            push_fmt_cfg(
                doc,
                FormatConfigIssue::Svgo(SvgoConfigIssue::RemoveScriptsDisabled),
                sp,
                String::from(
                    "`svgo.config` sets `removeScripts: false`, allowing `<script>` XSS in SVGs",
                ),
            );
        }
    }
    let mut cur = node.walk();
    for child in node.children(&mut cur) {
        scan_svgo_pairs(child, source, doc);
    }
}
