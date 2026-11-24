//! PowerShell `Set-StrictMode` & `$ErrorActionPreference = 'Stop'` header verifier (`PS-STRICT-HEADER`).

use gf_core::{LangRuleFinding, PowerShellViolation, RepoPath, Span};
use tree_sitter::Node;

use crate::cst::node_text;

/// Returns `true` if `path` requires `Set-StrictMode -Version Latest` & `$ErrorActionPreference = 'Stop'`.
#[must_use]
pub fn requires_ps_strict_header(path: &RepoPath) -> bool {
    let is_module_or_manifest = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psm1") || ext.eq_ignore_ascii_case("psd1"));
    let is_pester = path
        .file_name()
        .rsplit_once('.')
        .is_some_and(|(stem, _)| stem.to_ascii_lowercase().ends_with(".tests"));
    !is_module_or_manifest && !is_pester
}

#[derive(Clone, Copy, Default)]
struct PsHeaderFlags {
    strict_mode: bool,
    eap_stop: bool,
}

fn is_strict_mode_latest_cmd(node: Node<'_>, source: &str) -> bool {
    let Some(cmd_name) = node
        .children(&mut node.walk())
        .find(|n| n.kind() == "command_name")
    else {
        return false;
    };
    if !node_text(cmd_name, source)
        .trim()
        .eq_ignore_ascii_case("Set-StrictMode")
    {
        return false;
    }
    let lower = node_text(node, source).to_ascii_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    tokens
        .windows(2)
        .any(|w| (w[0] == "-version" || w[0] == "-v") && matches!(w[1], "latest" | "3" | "3.0"))
}

fn is_eap_stop_assign(node: Node<'_>, source: &str) -> bool {
    let Some((lhs, rhs)) = node_text(node, source).split_once('=') else {
        return false;
    };
    let lhs_clean = lhs.trim().to_ascii_lowercase();
    let rhs_clean = rhs.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    matches!(
        lhs_clean.as_str(),
        "$erroractionpreference"
            | "$script:erroractionpreference"
            | "$global:erroractionpreference"
    ) && rhs_clean == "stop"
}

fn scan_top_level_header(node: Node<'_>, source: &str, flags: &mut PsHeaderFlags) {
    if matches!(
        node.kind(),
        "function_statement" | "if_statement" | "try_statement"
    ) {
        return;
    }
    if node.kind() == "command" && is_strict_mode_latest_cmd(node, source) {
        flags.strict_mode = true;
    } else if node.kind() == "assignment_expression" && is_eap_stop_assign(node, source) {
        flags.eap_stop = true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        scan_top_level_header(child, source, flags);
    }
}

/// Emits `PS-STRICT-HEADER` if an executable `.ps1` script omits strict mode or `Stop` preference.
pub fn check_ps_strict_header(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut flags = PsHeaderFlags::default();
    scan_top_level_header(root, source, &mut flags);
    if !flags.strict_mode || !flags.eap_stop {
        out.push(LangRuleFinding::new(
            PowerShellViolation::MissingStrictHeader,
            Span::FIRST,
            "executable `.ps1` script must set both `Set-StrictMode -Version Latest` and `$ErrorActionPreference = 'Stop'`",
        ));
    }
}
