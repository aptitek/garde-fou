//! Cross-AST bridge from Docker Compose `command` / `entrypoint` (`sh -c` / `bash -c`) and
//! `healthcheck.test` (`CMD-SHELL`) to Tier 1 POSIX/Bash `Tree-sitter` analyzers.

use gf_core::{LangRuleFinding, ShellDialect, Span};

use super::anchors_merge::ResolvedYaml;
use super::events_tree::YamlScalar;
use super::yaml_ast::is_extension_key;
use crate::langs::shell::analyze_embedded_shell_snippet;

fn shell_dialect_for_bin(bin: &str) -> Option<ShellDialect> {
    let base = bin.rsplit('/').next().unwrap_or(bin);
    match base {
        "sh" | "dash" | "ash" => Some(ShellDialect::PosixSh),
        "bash" => Some(ShellDialect::Bash),
        _ => None,
    }
}

fn is_dash_c_flag(arg: &str) -> bool {
    !arg.starts_with("--") && arg.strip_prefix('-').is_some_and(|f| f.ends_with('c'))
}

fn extract_exec_shell_script(items: &[ResolvedYaml]) -> Option<(ShellDialect, &YamlScalar)> {
    let first = items.first()?.as_scalar()?;
    let dialect = shell_dialect_for_bin(first.value.trim())?;
    for (idx, item) in items.iter().enumerate().skip(1) {
        let Some(flag) = item.as_scalar() else {
            continue;
        };
        if is_dash_c_flag(flag.value.trim()) {
            let script = items.get(idx + 1)?.as_scalar()?;
            return Some((dialect, script));
        }
    }
    None
}

fn remap_finding_span(scalar: &YamlScalar, inner: Span) -> Span {
    let start_line = scalar.remap_embedded_line(inner.start_line());
    let end_line = scalar.remap_embedded_line(inner.end_line()).max(start_line);
    Span::new(start_line, end_line).unwrap_or(scalar.span)
}

fn run_compose_shell_snippet(
    dialect: ShellDialect,
    scalar: &YamlScalar,
    raw_script: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let unescaped = raw_script.replace("$$", "$");
    let Ok(bridged) = analyze_embedded_shell_snippet(dialect, &unescaped, false) else {
        return;
    };
    for mut finding in bridged.findings {
        finding.span = remap_finding_span(scalar, finding.span);
        out.push(finding);
    }
}

fn bridge_command_field(node: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    if let ResolvedYaml::Sequence { items, .. } = node
        && let Some((dialect, script)) = extract_exec_shell_script(items)
    {
        run_compose_shell_snippet(dialect, script, &script.value, out);
    }
}

fn bridge_healthcheck_seq(items: &[ResolvedYaml], out: &mut Vec<LangRuleFinding>) {
    if let Some(first) = items.first().and_then(ResolvedYaml::as_scalar)
        && first.value.trim() == "CMD-SHELL"
        && let Some(script) = items.get(1).and_then(ResolvedYaml::as_scalar)
    {
        run_compose_shell_snippet(ShellDialect::PosixSh, script, &script.value, out);
        return;
    }
    let tail = if items
        .first()
        .and_then(ResolvedYaml::as_scalar)
        .is_some_and(|f| f.value.trim() == "CMD")
    {
        &items[1..]
    } else {
        items
    };
    if let Some((dialect, script)) = extract_exec_shell_script(tail) {
        run_compose_shell_snippet(dialect, script, &script.value, out);
    }
}

fn bridge_healthcheck_test(hc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    let Some(test_node) = hc.get("test") else {
        return;
    };
    match test_node {
        ResolvedYaml::Sequence { items, .. } => bridge_healthcheck_seq(items, out),
        ResolvedYaml::Scalar(s) => {
            let trimmed = s.value.trim_start();
            if let Some(rest) = trimmed.strip_prefix("CMD-SHELL")
                && rest.starts_with(char::is_whitespace)
            {
                run_compose_shell_snippet(
                    ShellDialect::PosixSh,
                    s,
                    rest.trim_start_matches(' '),
                    out,
                );
            }
        }
        ResolvedYaml::Mapping { .. } => {}
    }
}

/// Bridges embedded shell scripts inside `command:`, `entrypoint:`, and `healthcheck.test:`.
pub fn bridge_compose_service_shells(
    services: &[(YamlScalar, ResolvedYaml)],
    out: &mut Vec<LangRuleFinding>,
) {
    for (k, svc) in services {
        if is_extension_key(&k.value) {
            continue;
        }
        if let Some(cmd) = svc.get("command") {
            bridge_command_field(cmd, out);
        }
        if let Some(ep) = svc.get("entrypoint") {
            bridge_command_field(ep, out);
        }
        if let Some(hc) = svc.get("healthcheck") {
            bridge_healthcheck_test(hc, out);
        }
    }
}
