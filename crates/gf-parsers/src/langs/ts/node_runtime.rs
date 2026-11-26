//! Modern Node.js runtime CST analyzer (`NODE-PROTOCOL-IMPORT`, `NODE-NO-SYNC-IO-IN-HANDLER`,
//! `NODE-MODERN-ESM-HYGIENE`).

use gf_core::{DomainKind, LangRuleFinding, NodeViolation, RepoPath, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

/// Sorted list of Node.js built-in module names (Biome + Node 22+ `sea`/`sqlite`/`test`).
#[rustfmt::skip]
const NODE_BUILTINS: &[&str] = &[
    "assert", "assert/strict", "async_hooks", "buffer", "child_process", "cluster", "console",
    "constants", "crypto", "dgram", "diagnostics_channel", "dns", "dns/promises", "domain",
    "events", "fs", "fs/promises", "http", "http2", "https", "inspector", "inspector/promises",
    "module", "net", "os", "path", "path/posix", "path/win32", "perf_hooks", "process",
    "punycode", "querystring", "readline", "readline/promises", "repl", "sea", "sqlite",
    "stream", "stream/consumers", "stream/promises", "stream/web", "string_decoder", "sys",
    "test", "test/reporters", "timers", "timers/promises", "tls", "trace_events", "tty", "url",
    "util", "util/types", "v8", "vm", "wasi", "worker_threads", "zlib",
];

#[rustfmt::skip]
const BROWSER_POLYFILLED_BUILTINS: &[&str] = &[
    "buffer", "events", "process", "punycode", "querystring", "stream", "string_decoder", "url", "util",
];

#[rustfmt::skip]
const BLOCKING_SYNC_CALLS: &[&str] = &[
    "readFileSync", "writeFileSync", "appendFileSync", "readdirSync", "statSync", "existsSync",
    "mkdirSync", "rmSync", "execSync", "spawnSync", "pbkdf2Sync", "scryptSync", "gzipSync",
];

/// Returns `true` if `specifier` is a Node.js built-in module missing the `node:` prefix.
#[must_use]
fn is_bare_node_builtin(specifier: &str) -> bool {
    let clean = specifier.trim().trim_matches(['"', '\'', '`']);
    !clean.starts_with("node:") && NODE_BUILTINS.binary_search(&clean).is_ok()
}

fn push_node(out: &mut Vec<LangRuleFinding>, v: NodeViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn is_cli_or_entry(path: &RepoPath) -> bool {
    let raw = path.as_str();
    let name = path.file_name();
    raw.starts_with("scripts/")
        || raw.starts_with("bin/")
        || raw.contains("/bin/")
        || matches!(
            name,
            "server.ts" | "server.js" | "main.ts" | "main.js" | "cli.ts" | "cli.js"
        )
}

fn is_async_or_handler(node: Node<'_>, source: &str) -> bool {
    matches!(
        node.kind(),
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    ) && {
        let txt = node_text(node, source);
        let head = txt.split('{').next().unwrap_or(txt);
        head.trim_start().starts_with("async ")
            || head.contains(" async ")
            || (head.contains("req") && head.contains("res"))
    }
}

fn check_protocol_specifier(
    node: Node<'_>,
    specifier: &str,
    is_jsx: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let clean = specifier.trim().trim_matches(['"', '\'', '`']);
    if !is_bare_node_builtin(clean) {
        return;
    }
    if is_jsx && BROWSER_POLYFILLED_BUILTINS.contains(&clean) {
        return;
    }
    push_node(
        out,
        NodeViolation::ProtocolImport,
        node,
        format!("import Node.js built-in `{clean}` with the explicit `node:{clean}` protocol"),
    );
}

const NF_JSX: u8 = 1 << 0;
const NF_IN_CODE: u8 = 1 << 1;
const NF_ENTRY: u8 = 1 << 2;
const NF_ESM_EXT: u8 = 1 << 3;
const NF_ASYNC: u8 = 1 << 4;

#[derive(Clone, Copy)]
struct NodeWalkCtx<'a> {
    source: &'a str,
    flags: u8,
}

impl NodeWalkCtx<'_> {
    const fn is_jsx(self) -> bool {
        (self.flags & NF_JSX) != 0
    }
    const fn in_code(self) -> bool {
        (self.flags & NF_IN_CODE) != 0
    }
    const fn is_entry(self) -> bool {
        (self.flags & NF_ENTRY) != 0
    }
    const fn is_esm_ext(self) -> bool {
        (self.flags & NF_ESM_EXT) != 0
    }
    const fn in_async(self) -> bool {
        (self.flags & NF_ASYNC) != 0
    }
    const fn with_async(self) -> Self {
        Self {
            source: self.source,
            flags: self.flags | NF_ASYNC,
        }
    }
}

fn inspect_require_or_import(
    node: Node<'_>,
    ftxt: &str,
    ctx: NodeWalkCtx<'_>,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(args) = node.child_by_field_name("arguments") {
        let arg_txt = node_text(args, ctx.source)
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim();
        check_protocol_specifier(node, arg_txt, ctx.is_jsx(), out);
    }
    if ftxt == "require" && ctx.is_esm_ext() && ctx.in_code() {
        push_node(
            out,
            NodeViolation::ModernEsmHygiene,
            node,
            String::from("avoid CommonJS `require(...)` in ESM TypeScript/JS; use static `import`"),
        );
    }
}

fn inspect_call_expr(node: Node<'_>, ctx: NodeWalkCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, ctx.source).trim();
    if ftxt == "require" || ftxt == "import" {
        inspect_require_or_import(node, ftxt, ctx, out);
    }
    let method = ftxt.rsplit('.').next().unwrap_or(ftxt);
    if ctx.in_async() && BLOCKING_SYNC_CALLS.contains(&method) {
        push_node(
            out,
            NodeViolation::NoSyncIoInHandler,
            node,
            format!(
                "blocking synchronous call `{method}(...)` inside async/handler blocks the Node.js event loop"
            ),
        );
    }
    if ctx.in_code() && !ctx.is_entry() && ftxt == "process.exit" {
        push_node(
            out,
            NodeViolation::ModernEsmHygiene,
            node,
            String::from(
                "avoid `process.exit(...)` in library modules; throw or return an error to the entrypoint",
            ),
        );
    }
}

fn walk_node(node: Node<'_>, ctx: NodeWalkCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "import_statement" | "export_statement" => {
            if let Some(src_node) = node.child_by_field_name("source") {
                check_protocol_specifier(node, node_text(src_node, ctx.source), ctx.is_jsx(), out);
            }
        }
        "call_expression" => inspect_call_expr(node, ctx, out),
        "new_expression"
            if node
                .child_by_field_name("constructor")
                .is_some_and(|c| node_text(c, ctx.source).trim() == "Buffer") =>
        {
            push_node(
                out,
                NodeViolation::ModernEsmHygiene,
                node,
                String::from(
                    "deprecated `new Buffer(...)` is unsafe; use `Buffer.from(...)` or `Buffer.alloc(...)`",
                ),
            );
        }
        _ => {}
    }
    let next_ctx = if is_async_or_handler(node, ctx.source) {
        ctx.with_async()
    } else {
        ctx
    };
    let mut cursor = node.walk();
    for ch in node.named_children(&mut cursor) {
        walk_node(ch, next_ctx, out);
    }
}

/// Extracts modern Node.js runtime findings (`NODE-*`) from a TS/JS CST.
pub fn extract_node_runtime_rules(
    root: Node<'_>,
    source: &str,
    domain: DomainKind,
    doc: &mut UastDocument,
) {
    let ext = doc.path.extension().unwrap_or("");
    let is_jsx = ext.eq_ignore_ascii_case("tsx") || ext.eq_ignore_ascii_case("jsx");
    let is_esm_ext = ext.eq_ignore_ascii_case("ts")
        || ext.eq_ignore_ascii_case("mts")
        || ext.eq_ignore_ascii_case("mjs");
    let flags = (u8::from(is_jsx) * NF_JSX)
        | (u8::from(domain == DomainKind::Code) * NF_IN_CODE)
        | (u8::from(is_cli_or_entry(&doc.path)) * NF_ENTRY)
        | (u8::from(is_esm_ext) * NF_ESM_EXT);
    walk_node(
        root,
        NodeWalkCtx { source, flags },
        &mut doc.lang_rule_findings,
    );
}
