//! Tier 2 configuration auditors (`GF081`) for `.shellcheckrc` and `.hadolint.yaml`.

use gf_core::{
    HadolintConfigIssue, ShellInfraConfigIssue, ShellcheckConfigIssue, Span, ToolConfigFinding,
    ToolConfigViolation,
};

use super::jsonc::push_cfg_violation;

const CRITICAL_HADOLINT_RULES: &[&str] = &["dl3002", "dl3007", "dl4006", "sc2086"];

fn line_span(line_idx_zero_based: usize) -> Span {
    let line = u32::try_from(line_idx_zero_based)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    Span::new(line, line).unwrap_or(Span::FIRST)
}

fn parse_sc_code_num(raw: &str) -> Option<u32> {
    let digits = raw
        .trim()
        .strip_prefix("SC")
        .or_else(|| raw.trim().strip_prefix("sc"))
        .unwrap_or_else(|| raw.trim());
    digits.parse::<u32>().ok()
}

fn is_global_sc_range(item: &str) -> bool {
    item.split_once('-')
        .and_then(|(lo, hi)| Some((parse_sc_code_num(lo)?, parse_sc_code_num(hi)?)))
        .is_some_and(|(a, b)| b > a)
}

fn audit_shellcheck_disable_item(item: &str, span: Span, out: &mut Vec<ToolConfigFinding>) {
    if item.eq_ignore_ascii_case("all") {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::Shellcheck(
                ShellcheckConfigIssue::DisableAll,
            )),
            span,
            String::from("`disable=all` in `.shellcheckrc` silences all ShellCheck checks"),
        );
    } else if is_global_sc_range(item) {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::Shellcheck(
                ShellcheckConfigIssue::GlobalRangeDisable,
            )),
            span,
            format!(
                "global range `disable={item}` in `.shellcheckrc` masks broad ShellCheck rules"
            ),
        );
    }
}

/// Audits `.shellcheckrc` / `shellcheckrc` for `disable=all` or `disable=SCxxxx-SCyyyy` ranges.
pub fn audit_shellcheckrc(source: &str, out: &mut Vec<ToolConfigFinding>) {
    for (idx, raw_line) in source.lines().enumerate() {
        let without_comment = raw_line.split('#').next().unwrap_or("").trim();
        let Some((key, val)) = without_comment.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("disable") {
            continue;
        }
        let span = line_span(idx);
        for item in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            audit_shellcheck_disable_item(item, span, out);
        }
    }
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return &line[..i],
            _ => {}
        }
    }
    line
}

fn clean_yaml_scalar(raw: &str) -> &str {
    raw.trim().trim_matches(['"', '\'']).trim()
}

fn check_hadolint_item(section: &str, item: &str, span: Span, out: &mut Vec<ToolConfigFinding>) {
    let val = clean_yaml_scalar(item);
    if section == "ignored" && CRITICAL_HADOLINT_RULES.contains(&val.to_ascii_lowercase().as_str())
    {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::Hadolint(
                HadolintConfigIssue::CriticalRuleIgnored,
            )),
            span,
            format!("critical rule `{val}` silenced in `.hadolint.yaml` `ignored:`"),
        );
    } else if section == "trustedRegistries" && val == "*" {
        push_cfg_violation(
            out,
            ToolConfigViolation::ShellInfra(ShellInfraConfigIssue::Hadolint(
                HadolintConfigIssue::WildcardTrustedRegistry,
            )),
            span,
            String::from("`trustedRegistries: [\"*\"]` disables container registry allowlisting"),
        );
    }
}

struct HadolintKeyLine<'a> {
    key: &'a str,
    rhs: &'a str,
    indent: usize,
    span: Span,
}

fn inspect_hadolint_key_rhs(
    line: &HadolintKeyLine<'_>,
    out: &mut Vec<ToolConfigFinding>,
) -> Option<(&'static str, usize)> {
    let sec = match line.key {
        "ignored" => "ignored",
        "trustedRegistries" => "trustedRegistries",
        _ => return None,
    };
    if let Some(flow) = line.rhs.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        for elem in flow.split(',') {
            check_hadolint_item(sec, elem, line.span, out);
        }
        None
    } else if line.rhs.is_empty() {
        Some((sec, line.indent))
    } else {
        check_hadolint_item(sec, line.rhs, line.span, out);
        None
    }
}

fn process_hadolint_line(
    uncommented: &str,
    span: Span,
    active_section: &mut Option<(&'static str, usize)>,
    out: &mut Vec<ToolConfigFinding>,
) {
    let trimmed = uncommented.trim();
    if trimmed.is_empty() || trimmed == "---" {
        return;
    }
    let indent = uncommented
        .len()
        .saturating_sub(uncommented.trim_start().len());
    if let Some((sec, sec_indent)) = *active_section {
        if let Some(seq_item) = trimmed.strip_prefix('-').filter(|_| indent >= sec_indent) {
            check_hadolint_item(sec, seq_item, span, out);
            return;
        }
        if indent <= sec_indent {
            *active_section = None;
        }
    }
    if let Some((key_raw, rhs_raw)) = trimmed.split_once(':') {
        let key_line = HadolintKeyLine {
            key: clean_yaml_scalar(key_raw),
            rhs: rhs_raw.trim(),
            indent,
            span,
        };
        if let Some(next_sec) = inspect_hadolint_key_rhs(&key_line, out) {
            *active_section = Some(next_sec);
        }
    }
}

/// Audits `.hadolint.yaml` / `hadolint.yaml` for critical `ignored:` rules and wildcard `trustedRegistries`.
pub fn audit_hadolint_yaml(source: &str, out: &mut Vec<ToolConfigFinding>) {
    let mut active_section: Option<(&'static str, usize)> = None;
    for (idx, raw_line) in source.lines().enumerate() {
        let uncommented = strip_yaml_comment(raw_line);
        process_hadolint_line(uncommented, line_span(idx), &mut active_section, out);
    }
}
