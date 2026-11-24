//! Native YAML 1.2 vs YAML 1.1 lexical quirk checks (`YAML-TRUTHY`, `YAML-IMPLICIT-OCTAL`,
//! `YAML-SEXAGESIMAL`) and inline comment anti-bypass (`GF070`).

use gf_core::{BypassFinding, LangRuleFinding, Span, YamlFlavor, YamlViolation};

use super::events_tree::{RawYamlStreamDoc, YamlNode, YamlScalar, YamlStyle};

const AMBIGUOUS_TRUTHY: &[&str] = &[
    "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF", "y", "Y", "n",
    "N", "True", "TRUE", "False", "FALSE",
];

fn is_ambiguous_truthy(val: &str) -> bool {
    AMBIGUOUS_TRUTHY.contains(&val)
}

fn is_implicit_octal(val: &str) -> bool {
    val.len() >= 2 && val.starts_with('0') && val.bytes().all(|b| matches!(b, b'0'..=b'7'))
}

const fn is_base60_slice(seg: &str) -> bool {
    match seg.as_bytes() {
        [d] => d.is_ascii_digit(),
        [tens, ones] => matches!(tens, b'0'..=b'5') && ones.is_ascii_digit(),
        _ => false,
    }
}

/// Matches `^\d+:[0-5]?\d(:[0-5]?\d)?$` (YAML 1.1 base-60 sexagesimal trap such as `22:22`).
fn is_sexagesimal(val: &str) -> bool {
    let mut parts = val.split(':');
    let Some(head) = parts.next() else {
        return false;
    };
    if head.is_empty() || !head.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Some(mid) = parts.next() else {
        return false;
    };
    if !is_base60_slice(mid) {
        return false;
    }
    parts
        .next()
        .is_none_or(|tail| is_base60_slice(tail) && parts.next().is_none())
}

fn check_plain_scalar(
    scalar: &YamlScalar,
    is_gha_root_key: bool,
    findings: &mut Vec<LangRuleFinding>,
) {
    if scalar.style != YamlStyle::Plain || scalar.has_tag {
        return;
    }
    let v = scalar.value.as_str();
    if is_gha_root_key && v == "on" {
        return;
    }
    if is_ambiguous_truthy(v) {
        findings.push(LangRuleFinding::new(
            YamlViolation::TruthyAmbiguity,
            scalar.span,
            format!(
                "ambiguous YAML 1.1 truthy scalar `{v}`; use `true`/`false` or quote `\"{v}\"`"
            ),
        ));
    } else if is_implicit_octal(v) {
        findings.push(LangRuleFinding::new(
            YamlViolation::ImplicitOctal,
            scalar.span,
            format!("implicit YAML 1.1 octal `{v}` evaluates to base-10 in YAML 1.2; quote `\"{v}\"` or write `0o{}`", &v[1..]),
        ));
    } else if is_sexagesimal(v) {
        findings.push(LangRuleFinding::new(
            YamlViolation::SexagesimalValue,
            scalar.span,
            format!("unquoted sexagesimal `{v}` is coerced to a base-60 integer by YAML 1.1 parsers; quote `\"{v}\"`"),
        ));
    }
}

fn walk_raw_quirks(
    node: &YamlNode,
    is_root: bool,
    flavor: YamlFlavor,
    findings: &mut Vec<LangRuleFinding>,
) {
    match node {
        YamlNode::Scalar(s) => check_plain_scalar(s, false, findings),
        YamlNode::Sequence { items, .. } => {
            for item in items {
                walk_raw_quirks(item, false, flavor, findings);
            }
        }
        YamlNode::Mapping { entries, .. } => {
            let is_gha_root = is_root && flavor == YamlFlavor::GithubWorkflow;
            for (k, v) in entries {
                if let YamlNode::Scalar(ks) = k {
                    check_plain_scalar(ks, is_gha_root, findings);
                } else {
                    walk_raw_quirks(k, false, flavor, findings);
                }
                walk_raw_quirks(v, false, flavor, findings);
            }
        }
        YamlNode::Alias { .. } => {}
    }
}

/// Evaluates `YAML-TRUTHY`, `YAML-IMPLICIT-OCTAL`, and `YAML-SEXAGESIMAL` on a raw YAML document.
pub fn check_yaml_scalar_quirks(
    doc: &RawYamlStreamDoc,
    flavor: YamlFlavor,
    findings: &mut Vec<LangRuleFinding>,
) {
    if let Some(root) = &doc.root {
        walk_raw_quirks(root, true, flavor, findings);
    }
}

fn find_yaml_line_comment(line: &str) -> Option<&str> {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match b {
            b'\\' if in_double => escaped = true,
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single
                && !in_double
                && (idx == 0 || bytes[idx - 1].is_ascii_whitespace()) =>
            {
                return line.get(idx + 1..);
            }
            _ => {}
        }
    }
    None
}

fn has_justification_reason(rest: &str) -> bool {
    rest.split_once("--")
        .is_some_and(|(_, reason)| reason.trim().chars().count() >= 10)
}

fn is_single_rule_token(tok: &str) -> bool {
    !tok.is_empty() && !tok.contains(',') && !tok.contains(' ') && !tok.eq_ignore_ascii_case("all")
}

fn is_valid_yamllint_disable_line(after_prefix: &str) -> bool {
    has_justification_reason(after_prefix)
        && after_prefix
            .split_once("--")
            .and_then(|(spec, _)| spec.trim().strip_prefix("rule:"))
            .is_some_and(is_single_rule_token)
}

fn classify_yamllint_pragma(rest: &str) -> Option<&'static str> {
    let r = rest.trim_start();
    if let Some(after_line) = r.strip_prefix("disable-line") {
        return (!is_valid_yamllint_disable_line(after_line)).then_some("# yamllint disable-line");
    }
    r.starts_with("disable").then_some("# yamllint disable")
}

fn classify_dclint_pragma(rest: &str) -> Option<&'static str> {
    let r = rest.trim_start();
    if let Some(after_line) = r.strip_prefix("disable-line") {
        let valid = has_justification_reason(after_line)
            && after_line
                .split_once("--")
                .is_some_and(|(rule, _)| is_single_rule_token(rule.trim()));
        return (!valid).then_some("# dclint disable-line");
    }
    r.starts_with("disable").then_some("# dclint disable")
}

fn classify_zizmor_pragma(after_ziz: &str) -> Option<&'static str> {
    let z = after_ziz.trim_start();
    if let Some(after_ig) = z.strip_prefix("ignore[") {
        let valid = after_ig.split_once(']').is_some_and(|(rules, tail)| {
            is_single_rule_token(rules.trim()) && has_justification_reason(tail)
        });
        return (!valid).then_some("# zizmor: ignore");
    }
    z.starts_with("ignore").then_some("# zizmor: ignore")
}

fn classify_yaml_comment_bypass(comment_body: &str) -> Option<&'static str> {
    let trimmed = comment_body.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("yamllint ") {
        return classify_yamllint_pragma(rest);
    }
    if let Some(rest) = lower.strip_prefix("dclint ") {
        return classify_dclint_pragma(rest);
    }
    if lower.starts_with("actionlint ignore") {
        return (!has_justification_reason(trimmed)).then_some("# actionlint ignore");
    }
    lower
        .strip_prefix("zizmor:")
        .and_then(classify_zizmor_pragma)
}

/// Scans YAML `#` comments (excluding block scalar bodies) for unjustified linter suppressions (`GF070`).
pub fn scan_yaml_comment_bypasses(
    source: &str,
    block_ranges: &[(u32, u32)],
    bypasses: &mut Vec<BypassFinding>,
) {
    for (idx, line) in source.lines().enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(1);
        if block_ranges
            .iter()
            .any(|&(start, end)| line_no >= start && line_no <= end)
        {
            continue;
        }
        if let Some(comment) = find_yaml_line_comment(line)
            && let Some(directive) = classify_yaml_comment_bypass(comment)
        {
            let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
            bypasses.push(BypassFinding {
                directive: String::from(directive),
                span,
            });
        }
    }
}
