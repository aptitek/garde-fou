//! POSIX `#!/bin/sh` bashism detector (`SH-SC3000`, faithful to `ShellCheck` `ShellSupport.hs`)
//! and CST pipeline detector for `DK-DL4006`.

use gf_core::{LangRuleFinding, ShellViolation};
use tree_sitter::Node;

use super::strict_mode::{StrictFlags, update_set_flags};
use crate::cst::{node_span, node_text};

fn is_bash_param_expansion(raw: &str) -> bool {
    let Some(inner) = raw.strip_prefix("${").and_then(|s| s.strip_suffix('}')) else {
        return false;
    };
    if inner.starts_with('!') {
        return true;
    }
    let name_len = inner
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    if name_len == 0 {
        return false;
    }
    let rest = &inner[name_len..];
    if rest.starts_with('/') || rest.starts_with('^') || rest.starts_with(',') {
        return true;
    }
    if let Some(after_colon) = rest.strip_prefix(':') {
        return !after_colon.starts_with(['-', '=', '?', '+']);
    }
    false
}

fn is_in_if_condition(node: Node<'_>) -> bool {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if matches!(parent.kind(), "if_statement" | "elif_clause") {
            let mut c = parent.walk();
            let then_byte = parent
                .children(&mut c)
                .find(|n| !n.is_named() && n.kind() == "then")
                .map_or(0, |t| t.start_byte());
            return cur.end_byte() <= then_byte;
        }
        cur = parent;
    }
    false
}

fn check_posix_command_bashism(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let name = node_text(name_node, source).trim();
    let mut cursor = node.walk();
    let args: Vec<Node<'_>> = node
        .children_by_field_name("argument", &mut cursor)
        .collect();
    match name {
        "source" => out.push(LangRuleFinding::new(
            ShellViolation::PosixBashism,
            node_span(node),
            "`source` is undefined in POSIX `sh`; use `. \"file\"` (`SC3046`)",
        )),
        "shopt" | "select" | "readarray" | "mapfile" => out.push(LangRuleFinding::new(
            ShellViolation::PosixBashism,
            node_span(node),
            format!("`{name}` is a Bash builtin unavailable in POSIX `sh`"),
        )),
        "echo"
            if args.first().is_some_and(|a| {
                matches!(
                    node_text(*a, source).trim(),
                    "-e" | "-n" | "-en" | "-ne" | "-E"
                )
            }) =>
        {
            out.push(LangRuleFinding::new(
                ShellViolation::PosixBashism,
                node_span(node),
                "`echo` flags (`-e`/`-n`) are non-portable in POSIX `sh`; use `printf` (`SC3037`)",
            ));
        }
        "set" => {
            let mut flags = StrictFlags::default();
            update_set_flags(&args, source, &mut flags);
            if flags.has_pipefail() && !is_in_if_condition(node) {
                out.push(LangRuleFinding::new(
                    ShellViolation::PosixBashism,
                    node_span(node),
                    "`set -o pipefail` is undefined in strict POSIX `sh` (`dash`) (`SC3040`)",
                ));
            }
        }
        _ => {}
    }
}

const SIMPLE_KIND_BASHISMS: &[(&str, &str)] = &[
    (
        "c_style_for_statement",
        "C-style `for ((;;))` loop is a Bash extension (`SC3004`)",
    ),
    (
        "array",
        "indexed/associative arrays are unavailable in POSIX `sh` (`SC3054`)",
    ),
    (
        "subscript",
        "indexed/associative arrays are unavailable in POSIX `sh` (`SC3054`)",
    ),
    (
        "herestring_redirect",
        "here-strings `<<<` are unavailable in POSIX `sh` (`SC3011`)",
    ),
    (
        "process_substitution",
        "process substitution `<(...)` is unavailable in POSIX `sh` (`SC3001`)",
    ),
    (
        "ansi_c_string",
        "`$'...'` / `$\"...\"` string literals are non-POSIX (`SC3003`)",
    ),
    (
        "translated_string",
        "`$'...'` / `$\"...\"` string literals are non-POSIX (`SC3003`)",
    ),
];

fn classify_node_bashism(node: Node<'_>, source: &str) -> Option<&'static str> {
    let kind = node.kind();
    if let Some((_, msg)) = SIMPLE_KIND_BASHISMS.iter().find(|(k, _)| *k == kind) {
        return Some(*msg);
    }
    let text = node_text(node, source).trim_start();
    match kind {
        "test_command" if text.starts_with("[[") => {
            Some("`[[ ... ]]` conditional is a Bash extension (`SC3010`); use `[ ... ]`")
        }
        "compound_statement" if text.starts_with("((") => {
            Some("`(( ... ))` arithmetic command is a Bash extension (`SC3006`)")
        }
        "function_definition"
            if text.starts_with("function ") || text.starts_with("function\t") =>
        {
            Some("`function` keyword is non-POSIX (`SC3013`); write `name() { ... }`")
        }
        "file_redirect" if text.starts_with("&>") => {
            Some("`&>` combined redirect is non-POSIX (`SC3020`); use `>file 2>&1`")
        }
        "pipeline" if node_text(node, source).contains("|&") => {
            Some("`|&` pipe shorthand is non-POSIX (`SC3023`); use `2>&1 |`")
        }
        "expansion" if is_bash_param_expansion(text) => {
            Some("Bash parameter expansion (`${var//a/b}`, `${var:0:3}`, `${!var}`) is non-POSIX")
        }
        "declaration_command"
            if matches!(
                text.split_whitespace().next(),
                Some("local" | "declare" | "typeset")
            ) =>
        {
            Some("`local`/`declare`/`typeset` is undefined in strict POSIX `sh` (`SC3043`)")
        }
        _ => None,
    }
}

/// Walks a `PosixSh` CST and emits `SH-SC3000` for any Bash-only construct.
pub fn check_posix_bashisms(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if let Some(msg) = classify_node_bashism(node, source) {
        out.push(LangRuleFinding::new(
            ShellViolation::PosixBashism,
            node_span(node),
            msg,
        ));
    } else if node.kind() == "command" {
        check_posix_command_bashism(node, source, out);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_posix_bashisms(child, source, out);
    }
}

fn subtree_has_pipeline(node: Node<'_>) -> bool {
    if node.kind() == "pipeline" {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(subtree_has_pipeline)
}

fn statement_enables_pipefail(node: Node<'_>, source: &str) -> bool {
    if node.kind() == "command"
        && node
            .child_by_field_name("name")
            .is_some_and(|n| node_text(n, source).trim() == "set")
    {
        let mut cursor = node.walk();
        let args: Vec<Node<'_>> = node
            .children_by_field_name("argument", &mut cursor)
            .collect();
        let mut flags = StrictFlags::default();
        update_set_flags(&args, source, &mut flags);
        return flags.has_pipefail();
    }
    if node.kind() == "list"
        && let Some(left) = node.named_child(0)
    {
        return statement_enables_pipefail(left, source);
    }
    false
}

/// Returns `true` if a `Dockerfile RUN` script contains a CST `pipeline` (`|`) before `set -o pipefail`.
#[must_use]
pub fn has_unguarded_shell_pipeline(root: Node<'_>, source: &str) -> bool {
    let mut pipefail_active = false;
    let mut cursor = root.walk();
    for stmt in root.children(&mut cursor) {
        if statement_enables_pipefail(stmt, source) {
            pipefail_active = true;
        }
        if !pipefail_active && subtree_has_pipeline(stmt) {
            return true;
        }
    }
    false
}
