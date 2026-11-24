//! Tier 2 static configuration auditor (`GF081`) for `.yamllint*`, `.dclintrc*`, and `zizmor.yml`.

use gf_core::{
    DclintConfigIssue, ToolConfigFinding, ToolConfigViolation, UastDocument, YamlCiConfigIssue,
    YamllintConfigIssue, ZizmorConfigIssue,
};

use super::anchors_merge::{ResolvedYaml, resolve_yaml_document};
use super::events_tree::parse_yaml_stream;
use crate::langs::jsonc::{push_cfg_violation, strip_jsonc_to_json};

const CRITICAL_DCLINT_RULES: &[&str] = &[
    "service-image-require-explicit-tag",
    "require-quotes-in-ports",
    "no-unbound-port-interfaces",
    "no-privileged-containers",
    "no-docker-sock",
];

const CRITICAL_ZIZMOR_RULES: &[&str] = &[
    "template-injection",
    "unpinned-uses",
    "excessive-permissions",
    "dangerous-triggers",
    "artipacked",
];

const WILDCARD_DCLINT_EXCLUDES: &[&str] = &["*", "**", "**/*", "*.yml", "*.yaml"];

fn is_disabled_scalar(val: &str) -> bool {
    matches!(
        val.trim().to_ascii_lowercase().as_str(),
        "disable" | "disabled" | "off" | "false" | "0"
    )
}

fn is_dclint_rule_disabled(node: &ResolvedYaml) -> bool {
    match node {
        ResolvedYaml::Scalar(s) => is_disabled_scalar(&s.value),
        ResolvedYaml::Sequence { items, .. } => items
            .first()
            .and_then(ResolvedYaml::as_scalar)
            .is_some_and(|s| is_disabled_scalar(&s.value)),
        ResolvedYaml::Mapping { .. } => node
            .get("level")
            .or_else(|| node.get("enabled"))
            .and_then(ResolvedYaml::as_scalar)
            .is_some_and(|s| is_disabled_scalar(&s.value)),
    }
}

fn parse_config_yaml(source: &str, doc: &mut UastDocument) -> Option<ResolvedYaml> {
    let stream = parse_yaml_stream(source);
    doc.syntax_errors.extend(stream.syntax_errors);
    let first = stream.docs.first()?;
    resolve_yaml_document(first, &mut doc.lang_rule_findings)
}

/// Audits `.yamllint`, `.yamllint.yml`, and `.yamllint.yaml` for weakened rules (`GF081`).
pub fn audit_yamllint_config(source: &str, doc: &mut UastDocument) {
    let Some(root) = parse_config_yaml(source, doc) else {
        return;
    };
    if let Some(ext) = root.get("extends").and_then(ResolvedYaml::as_scalar)
        && ext.value.trim().eq_ignore_ascii_case("relaxed")
    {
        push_cfg_violation(
            &mut doc.tool_config_findings,
            ToolConfigViolation::YamlCi(YamlCiConfigIssue::Yamllint(
                YamllintConfigIssue::ExtendsRelaxed,
            )),
            ext.span,
            String::from("`extends: relaxed` weakens core `.yamllint` strictness"),
        );
    }
    let Some(rules) = root.get("rules") else {
        return;
    };
    for (rule_key, issue) in [
        ("truthy", YamllintConfigIssue::TruthyDisabled),
        ("key-duplicates", YamllintConfigIssue::KeyDuplicatesDisabled),
    ] {
        if let Some(node) = rules.get(rule_key)
            && is_dclint_rule_disabled(node)
        {
            push_cfg_violation(
                &mut doc.tool_config_findings,
                ToolConfigViolation::YamlCi(YamlCiConfigIssue::Yamllint(issue)),
                node.span(),
                format!("`.yamllint` disables mandatory rule `{rule_key}`"),
            );
        }
    }
}

fn audit_dclint_excludes(root: &ResolvedYaml, out: &mut Vec<ToolConfigFinding>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = root.get("exclude") else {
        return;
    };
    for item in items {
        if let Some(s) = item.as_scalar() {
            let pat = s.value.trim().trim_start_matches("./");
            if WILDCARD_DCLINT_EXCLUDES.contains(&pat) {
                push_cfg_violation(
                    out,
                    ToolConfigViolation::YamlCi(YamlCiConfigIssue::Dclint(
                        DclintConfigIssue::WildcardExcluded,
                    )),
                    s.span,
                    format!(
                        "wildcard `exclude` pattern `{}` disables Compose linting",
                        s.value
                    ),
                );
            }
        }
    }
}

/// Audits `.dclintrc`, `.dclintrc.json`, `.dclintrc.yml`, and `.dclintrc.yaml` (`GF081`).
pub fn audit_dclint_config(file_name: &str, source: &str, doc: &mut UastDocument) {
    let normalized;
    let yaml_input = if file_name.to_ascii_lowercase().ends_with(".json")
        || source.trim_start().starts_with('{')
    {
        normalized = strip_jsonc_to_json(source);
        normalized.as_str()
    } else {
        source
    };
    let Some(root) = parse_config_yaml(yaml_input, doc) else {
        return;
    };
    audit_dclint_excludes(&root, &mut doc.tool_config_findings);
    let Some(ResolvedYaml::Mapping { entries, .. }) = root.get("rules") else {
        return;
    };
    for (k, v) in entries {
        let lower_k = k.value.trim().to_ascii_lowercase();
        if CRITICAL_DCLINT_RULES.contains(&lower_k.as_str()) && is_dclint_rule_disabled(v) {
            push_cfg_violation(
                &mut doc.tool_config_findings,
                ToolConfigViolation::YamlCi(YamlCiConfigIssue::Dclint(
                    DclintConfigIssue::CriticalRuleDisabled,
                )),
                k.span,
                format!("`.dclintrc` disables critical Compose rule `{}`", k.value),
            );
        }
    }
}

/// Audits `zizmor.yml` / `.github/zizmor.yml` for disabled workflow security audits (`GF081`).
pub fn audit_zizmor_config(source: &str, doc: &mut UastDocument) {
    let Some(root) = parse_config_yaml(source, doc) else {
        return;
    };
    let Some(ResolvedYaml::Mapping { entries, .. }) = root.get("rules") else {
        return;
    };
    for (k, v) in entries {
        let lower_k = k.value.trim().to_ascii_lowercase();
        if CRITICAL_ZIZMOR_RULES.contains(&lower_k.as_str())
            && let Some(dis) = v.get("disable").and_then(ResolvedYaml::as_scalar)
            && dis.value.trim().eq_ignore_ascii_case("true")
        {
            push_cfg_violation(
                &mut doc.tool_config_findings,
                ToolConfigViolation::YamlCi(YamlCiConfigIssue::Zizmor(
                    ZizmorConfigIssue::CriticalRuleDisabled,
                )),
                dis.span,
                format!(
                    "`zizmor.yml` sets `disable: true` on critical audit `{}`",
                    k.value
                ),
            );
        }
    }
}
