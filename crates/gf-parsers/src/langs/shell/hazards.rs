//! Shell command hazards (`SH-SC2155`, `SH-SC2115`, `SH-SC2164`, `SH-NO-EVAL`).

use gf_core::{LangRuleFinding, ShellViolation};
use tree_sitter::Node;

use super::curl_pipe::check_curl_pipe_pipeline;
use crate::cst::{node_span, node_text};

fn collect_cmd_tokens<'a>(cmd_node: Node<'a>, source: &'a str) -> (Option<&'a str>, Vec<Node<'a>>) {
    let name = cmd_node
        .child_by_field_name("name")
        .map(|n| node_text(n, source).trim());
    let mut cursor = cmd_node.walk();
    let args: Vec<Node<'a>> = cmd_node
        .children_by_field_name("argument", &mut cursor)
        .collect();
    (name, args)
}

fn contains_command_sub(node: Node<'_>) -> bool {
    if node.kind() == "function_definition" {
        return false;
    }
    if node.kind() == "command_substitution" {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(contains_command_sub)
}

fn check_sc2155_declaration(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let raw = node_text(node, source);
    let kw = raw.split_whitespace().next().unwrap_or("");
    if !matches!(kw, "local" | "export" | "readonly" | "declare" | "typeset") {
        return;
    }
    let mut cursor = node.walk();
    let masks_sub = node.children(&mut cursor).any(|ch| {
        ch.kind() == "variable_assignment"
            && ch
                .child_by_field_name("value")
                .is_some_and(contains_command_sub)
    });
    if masks_sub {
        out.push(LangRuleFinding::new(
            ShellViolation::MaskedReturnCode,
            node_span(node),
            format!("`{kw}` combined with `$(...)` masks the command substitution's exit code"),
        ));
    }
}

fn is_recursive_rm_flag(tok: &str) -> bool {
    if tok == "--recursive" {
        return true;
    }
    tok.strip_prefix('-')
        .is_some_and(|s| !s.starts_with('-') && (s.contains('r') || s.contains('R')))
}

fn is_catastrophic_rm_operand(raw: &str) -> bool {
    let clean: String = raw.chars().filter(|&c| c != '"' && c != '\'').collect();
    if clean.contains(":?") || !clean.contains('$') {
        return false;
    }
    if let Some(after_slash) = clean.strip_prefix('/') {
        return after_slash.starts_with('$');
    }
    if !clean.starts_with('$') {
        return false;
    }
    let Some((_, tail)) = clean.split_once('/') else {
        return false;
    };
    tail.chars().all(|c| c == '/' || c == '*')
}

#[derive(Default)]
struct RmOptState {
    recursive: bool,
    end_of_opts: bool,
}

impl RmOptState {
    fn consume_option(&mut self, tok: &str) -> bool {
        if self.end_of_opts {
            return false;
        }
        if tok == "--" {
            self.end_of_opts = true;
            return true;
        }
        if tok.starts_with('-') {
            self.recursive |= is_recursive_rm_flag(tok);
            return true;
        }
        false
    }
}

fn find_catastrophic_recursive_rm<'a>(args: &[Node<'a>], source: &'a str) -> Option<&'a str> {
    let mut state = RmOptState::default();
    let mut bad_arg: Option<&'a str> = None;
    for &arg in args {
        let tok = node_text(arg, source).trim();
        if !state.consume_option(tok) && is_catastrophic_rm_operand(tok) {
            bad_arg = Some(tok);
        }
    }
    state.recursive.then_some(bad_arg).flatten()
}

fn check_sc2115_rm(
    cmd_node: Node<'_>,
    args: &[Node<'_>],
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(operand) = find_catastrophic_recursive_rm(args, source) {
        out.push(LangRuleFinding::new(
            ShellViolation::CatastrophicRm,
            node_span(cmd_node),
            format!(
                "recursive `rm` on `{operand}` expands to `/` or `/*` if the variable is empty"
            ),
        ));
    }
}

fn is_before_branch_body(cur: Node<'_>, parent: Node<'_>) -> bool {
    let marker = match parent.kind() {
        "if_statement" | "elif_clause" => "then",
        "while_statement" | "until_statement" => "do_group",
        _ => return false,
    };
    let mut cursor = parent.walk();
    parent
        .children(&mut cursor)
        .find(|n| n.kind() == marker)
        .is_some_and(|m| cur.end_byte() <= m.start_byte())
}

fn is_guarded_in_list(cur: Node<'_>, list_parent: Node<'_>) -> bool {
    let mut cursor = list_parent.walk();
    list_parent.children(&mut cursor).any(|ch| {
        !ch.is_named() && matches!(ch.kind(), "||" | "&&") && cur.end_byte() <= ch.start_byte()
    })
}

fn is_cd_guarded_or_in_condition(cmd_node: Node<'_>) -> bool {
    let mut cur = cmd_node;
    while let Some(parent) = cur.parent() {
        match parent.kind() {
            "function_definition" | "program" => return false,
            "list" if is_guarded_in_list(cur, parent) => return true,
            "if_statement" | "elif_clause" | "while_statement" | "until_statement"
                if is_before_branch_body(cur, parent) =>
            {
                return true;
            }
            _ => cur = parent,
        }
    }
    false
}

fn is_inside_shell_function(node: Node<'_>) -> bool {
    let mut cur = node.parent();
    while let Some(p) = cur {
        if p.kind() == "function_definition" {
            return true;
        }
        cur = p.parent();
    }
    false
}

/// Walks the Shell CST for `SH-SC2155`, `SH-SC2115`, `SH-SC2164`, and `SH-NO-EVAL`.
pub fn check_shell_hazards(
    node: Node<'_>,
    source: &str,
    top_errexit: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    match node.kind() {
        "pipeline" => check_curl_pipe_pipeline(node, source, out),
        "declaration_command" => check_sc2155_declaration(node, source, out),
        "command" => {
            let (name, args) = collect_cmd_tokens(node, source);
            match name {
                Some("eval") => out.push(LangRuleFinding::new(
                    ShellViolation::ForbiddenEval,
                    node_span(node),
                    "`eval` executes dynamically constructed shell code (`SC2294`)",
                )),
                Some("rm" | "/bin/rm") => check_sc2115_rm(node, &args, source, out),
                Some("cd" | "pushd" | "popd")
                    if (!top_errexit || is_inside_shell_function(node))
                        && !is_cd_guarded_or_in_condition(node) =>
                {
                    out.push(LangRuleFinding::new(
                        ShellViolation::UncheckedCd,
                        node_span(node),
                        "`cd` without `|| exit`/`|| return` continues in the wrong directory on failure",
                    ));
                }
                _ => {}
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_shell_hazards(child, source, top_errexit, out);
    }
}
