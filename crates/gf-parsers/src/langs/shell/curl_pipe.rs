//! Detection of unencrypted or unhardened `curl ... | sh` / `wget -O- ... | sh` pipelines
//! (`SH-CURL-PIPE-UNSAFE`).

use gf_core::{LangRuleFinding, ShellViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const SHELL_SINKS: &[&str] = &[
    "sh",
    "bash",
    "dash",
    "ash",
    "zsh",
    "/bin/sh",
    "/bin/bash",
    "/usr/bin/env",
];

fn is_shell_sink(cmd: Node<'_>, source: &str) -> bool {
    let Some(name_node) = cmd.child_by_field_name("name") else {
        return false;
    };
    let name = node_text(name_node, source).trim();
    if name == "env" || name == "/usr/bin/env" {
        let mut cursor = cmd.walk();
        return cmd
            .children_by_field_name("argument", &mut cursor)
            .any(|arg| {
                matches!(
                    node_text(arg, source).trim(),
                    "sh" | "bash" | "dash" | "zsh"
                )
            });
    }
    SHELL_SINKS.contains(&name)
}

fn has_short_flag(args: &[&str], flag: char, long_flag: &str) -> bool {
    args.iter().any(|&arg| {
        arg == long_flag || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains(flag))
    })
}

fn has_hardened_curl_flags(args: &[&str]) -> bool {
    [
        ('f', "--fail"),
        ('s', "--silent"),
        ('S', "--show-error"),
        ('L', "--location"),
    ]
    .iter()
    .all(|&(short, long)| has_short_flag(args, short, long))
}

fn has_insecure_http_url(args: &[&str]) -> bool {
    args.iter().any(|arg| {
        let cleaned = arg.trim_matches(|c| c == '"' || c == '\'');
        cleaned.starts_with("http://")
    })
}

/// Checks a Shell `pipeline` AST node for unsafe `curl|sh` or `wget|sh` execution.
pub fn check_curl_pipe_pipeline(pipeline: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut cursor = pipeline.walk();
    let cmds: Vec<Node<'_>> = pipeline
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "command")
        .collect();
    if cmds.len() < 2 {
        return;
    }
    let pipes_to_shell = cmds[1..].iter().any(|&c| is_shell_sink(c, source));
    if !pipes_to_shell {
        return;
    }
    let first = cmds[0];
    let Some(name_node) = first.child_by_field_name("name") else {
        return;
    };
    let downloader = node_text(name_node, source).trim();
    if !matches!(downloader, "curl" | "wget") {
        return;
    }
    let mut arg_cur = first.walk();
    let args: Vec<&str> = first
        .children_by_field_name("argument", &mut arg_cur)
        .map(|n| node_text(n, source).trim())
        .collect();

    if has_insecure_http_url(&args) {
        out.push(LangRuleFinding::new(
            ShellViolation::CurlPipeUnsafe,
            node_span(pipeline),
            format!("`{downloader} http://... | sh` downloads and executes over unencrypted HTTP"),
        ));
    } else if downloader == "curl" && !has_hardened_curl_flags(&args) {
        out.push(LangRuleFinding::new(
            ShellViolation::CurlPipeUnsafe,
            node_span(pipeline),
            "`curl ... | sh` must pass `-fsSL` (fail on HTTP errors, show errors, follow redirects)",
        ));
    }
}
