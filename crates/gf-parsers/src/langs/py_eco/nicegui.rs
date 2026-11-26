//! `NiceGUI` reactive `asyncio` & multiprocessing CST analyzer (`NICEGUI-*` under `GF080`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, NiceGuiViolation};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

#[rustfmt::skip]
const BLOCKING_PY_CALLS: &[&str] = &[
    "time.sleep", "requests.get", "requests.post", "requests.put", "requests.delete",
    "requests.request", "urllib.request.urlopen", "subprocess.run", "subprocess.call",
    "subprocess.check_output", "input",
];

fn push_ng(out: &mut Vec<LangRuleFinding>, v: NiceGuiViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn collect_worker_fn_names(root: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "call"
            && let Some(func) = n.child_by_field_name("function")
            && matches!(
                node_text(func, source).trim(),
                "run.io_bound" | "run.cpu_bound" | "nicegui.run.io_bound" | "nicegui.run.cpu_bound"
            )
            && let Some(args) = n.child_by_field_name("arguments")
        {
            let mut ac = args.walk();
            if let Some(first) = args.named_children(&mut ac).next()
                && first.kind() == "identifier"
            {
                names.insert(String::from(node_text(first, source).trim()));
            }
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
    names
}

fn check_worker_body(body: Node<'_>, source: &str, fn_name: &str, out: &mut Vec<LangRuleFinding>) {
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        let txt = node_text(n, source).trim();
        let is_ui_call = n.kind() == "call"
            && n.child_by_field_name("function")
                .is_some_and(|f| node_text(f, source).trim().starts_with("ui."));
        let is_storage = n.kind() == "attribute"
            && (txt.starts_with("app.storage.user")
                || txt.starts_with("app.storage.tab")
                || txt.starts_with("app.storage.client")
                || txt.starts_with("app.storage.browser"));
        if is_ui_call || is_storage {
            push_ng(
                out,
                NiceGuiViolation::WorkerContextViolation,
                n,
                format!(
                    "`{fn_name}` runs outside NiceGUI UI context; do not access `ui.*` or `app.storage.*`"
                ),
            );
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
}

fn check_ng_security(node: Node<'_>, ftxt: &str, args_txt: &str, out: &mut Vec<LangRuleFinding>) {
    let compact: String = args_txt.chars().filter(|c| !c.is_whitespace()).collect();
    if ftxt == "ui.html" && compact.contains("sanitize=False") {
        push_ng(
            out,
            NiceGuiViolation::SecurityXssAndSecret,
            node,
            String::from(
                "`ui.html(..., sanitize=False)` introduces XSS risk; keep sanitization enabled",
            ),
        );
    }
    if ftxt == "ui.run_javascript"
        && (args_txt.contains("f\"") || args_txt.contains("f'"))
        && !args_txt.contains("json.dumps")
    {
        push_ng(
            out,
            NiceGuiViolation::SecurityXssAndSecret,
            node,
            String::from(
                "interpolated `ui.run_javascript(f\"...\")` without `json.dumps` allows JS injection",
            ),
        );
    }
}

fn check_ng_call(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, source).trim();
    let args_txt = node
        .child_by_field_name("arguments")
        .map_or("", |a| node_text(a, source));
    if (flags & 1) != 0 && BLOCKING_PY_CALLS.contains(&ftxt) {
        push_ng(
            out,
            NiceGuiViolation::BlockingEventLoop,
            node,
            format!(
                "blocking call `{ftxt}(...)` stalls NiceGUI event loop; use async or `run.io_bound`"
            ),
        );
    }
    check_ng_security(node, ftxt, args_txt, out);
    if ftxt == "ui.run" && (flags & 2) != 0 {
        let compact: String = args_txt.chars().filter(|c| !c.is_whitespace()).collect();
        let missing = !compact.contains("storage_secret=");
        let literal = compact.contains("storage_secret=\"") || compact.contains("storage_secret='");
        if missing || literal {
            push_ng(
                out,
                NiceGuiViolation::SecurityXssAndSecret,
                node,
                String::from(
                    "`ui.run(...)` with `app.storage.user`/`browser` requires `storage_secret` from `os.environ`",
                ),
            );
        }
    }
}

fn check_top_level_ui(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut c = root.walk();
    for ch in root.named_children(&mut c) {
        if ch.kind() == "expression_statement" {
            let txt = node_text(ch, source).trim();
            if txt.starts_with("ui.") && !txt.starts_with("ui.run(") {
                push_ng(
                    out,
                    NiceGuiViolation::GlobalUiOrStateLeak,
                    ch,
                    String::from(
                        "top-level `ui.*` element alongside `@ui.page(...)` leaks across clients",
                    ),
                );
            }
        }
    }
}

fn check_worker_fns(
    root: Node<'_>,
    source: &str,
    workers: &BTreeSet<String>,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut c = root.walk();
    for ch in root.named_children(&mut c) {
        let fn_node = if ch.kind() == "decorated_definition" {
            ch.child_by_field_name("definition")
        } else {
            Some(ch)
        };
        if let Some(f) = fn_node
            && f.kind() == "function_definition"
            && let Some(body) = f.child_by_field_name("body")
        {
            let name = field_name_string(f, "name", "<fn>", source);
            let is_on_exc = ch.kind() == "decorated_definition"
                && node_text(ch, source).contains("on_exception");
            if workers.contains(&name) || is_on_exc {
                check_worker_body(body, source, &name, out);
            }
        }
    }
}

fn walk_ng(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let in_async_ui = (flags & 1) != 0;
    match node.kind() {
        "call" => check_ng_call(node, source, flags, out),
        "global_statement" if in_async_ui => push_ng(
            out,
            NiceGuiViolation::GlobalUiOrStateLeak,
            node,
            String::from(
                "avoid `global` state in NiceGUI UI handlers; use `app.storage.user` / `tab`",
            ),
        ),
        _ => {}
    }
    let enters_ui = (node.kind() == "function_definition"
        && node_text(node, source)
            .trim_start()
            .starts_with("async def "))
        || (node.kind() == "decorated_definition" && node_text(node, source).contains("@ui.page"));
    let next_flags = if enters_ui { flags | 1 } else { flags };
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        walk_ng(ch, source, next_flags, out);
    }
}

/// Extracts `NiceGUI` CST findings (`NICEGUI-*`) without I/O.
pub fn extract_nicegui_rules(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if !source.contains("nicegui")
        && !source.contains("@ui.page")
        && !source.contains("run.io_bound")
    {
        return;
    }
    let workers = collect_worker_fn_names(root, source);
    if source.contains("@ui.page") {
        check_top_level_ui(root, source, out);
    }
    check_worker_fns(root, source, &workers, out);
    let uses_storage =
        source.contains("app.storage.user") || source.contains("app.storage.browser");
    walk_ng(root, source, u8::from(uses_storage) << 1, out);
}
