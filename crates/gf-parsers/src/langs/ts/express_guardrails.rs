//! Express 5 & API security CST guardrails (`EXPRESS-*` under `GF080`).

use gf_core::{LangRuleFinding, MernViolation, RepoPath, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const VALIDATOR_MARKERS: &[&str] = &[
    ".parse(",
    ".safeParse(",
    ".parseAsync(",
    ".safeParseAsync(",
    "validate(",
    "validateBody(",
    "validateQuery(",
    "validateParams(",
];

fn push_express(out: &mut Vec<LangRuleFinding>, v: MernViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn is_req_input_expr(txt: &str) -> bool {
    let t = txt.trim();
    t == "req.body"
        || t == "req.query"
        || t == "req.params"
        || t.starts_with("req.body.")
        || t.starts_with("req.query.")
        || t.starts_with("req.params.")
}

fn has_validator_call(body_txt: &str) -> bool {
    VALIDATOR_MARKERS.iter().any(|m| body_txt.contains(m))
}

fn check_cors_call(
    node: Node<'_>,
    args_txt: &str,
    has_auth_in_file: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let compact: String = args_txt.chars().filter(|c| !c.is_whitespace()).collect();
    let creds = compact.contains("credentials:true");
    let wildcard_or_reflect = compact.contains("origin:\"*\"")
        || compact.contains("origin:'*'")
        || compact.contains("origin:true");
    if (creds && wildcard_or_reflect) || (compact == "()" && has_auth_in_file) {
        push_express(
            out,
            MernViolation::ExpressInsecureCorsOrErrorLeak,
            node,
            String::from(
                "permissive `cors(...)` configuration exposes authenticated routes; restrict `origin` to an explicit allowlist",
            ),
        );
    }
}

fn check_response_leak(node: Node<'_>, ftxt: &str, args_txt: &str, out: &mut Vec<LangRuleFinding>) {
    let is_res_send = matches!(ftxt.rsplit('.').next(), Some("json" | "send"));
    let is_res_obj = ftxt.starts_with("res.") || ftxt.contains("res.status(");
    if is_res_send && is_res_obj && args_txt.contains(".stack") {
        push_express(
            out,
            MernViolation::ExpressInsecureCorsOrErrorLeak,
            node,
            String::from("do not expose internal error `.stack` traces in HTTP responses"),
        );
    }
}

fn is_app_module_with_routes(path: &RepoPath, source: &str) -> bool {
    if matches!(
        path.file_name(),
        "server.ts" | "server.js" | "main.ts" | "main.js"
    ) {
        return false;
    }
    let defines_routes = [
        "app.get(",
        "app.post(",
        "app.put(",
        "app.delete(",
        "app.use(",
    ]
    .iter()
    .any(|m| source.contains(m));
    let exports_app = source.contains("export const app")
        || source.contains("export default app")
        || source.contains("export { app");
    defines_routes && exports_app
}

const EF_AUTH: u8 = 1 << 0;
const EF_APP_MOD: u8 = 1 << 1;
const EF_VALIDATED: u8 = 1 << 2;

#[derive(Clone, Copy)]
struct ExpressCtx<'a> {
    source: &'a str,
    flags: u8,
}

impl ExpressCtx<'_> {
    const fn has_auth(self) -> bool {
        (self.flags & EF_AUTH) != 0
    }
    const fn is_app_mod(self) -> bool {
        (self.flags & EF_APP_MOD) != 0
    }
    const fn is_validated(self) -> bool {
        (self.flags & EF_VALIDATED) != 0
    }
    const fn set_validated(self, v: bool) -> Self {
        let flags = if v {
            self.flags | EF_VALIDATED
        } else {
            self.flags & !EF_VALIDATED
        };
        Self {
            source: self.source,
            flags,
        }
    }
}

fn check_express_call(node: Node<'_>, ctx: ExpressCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, ctx.source).trim();
    let args_txt = node
        .child_by_field_name("arguments")
        .map_or("", |a| node_text(a, ctx.source));
    if ftxt == "cors" {
        check_cors_call(node, args_txt, ctx.has_auth(), out);
    }
    check_response_leak(node, ftxt, args_txt, out);
    if ctx.is_app_mod() && ftxt.ends_with(".listen") {
        push_express(
            out,
            MernViolation::ExpressListenInAppModule,
            node,
            String::from(
                "do not call `app.listen(...)` inside the exported `app` module; move server binding to `server.ts`",
            ),
        );
    }
}

fn check_express_node(node: Node<'_>, ctx: ExpressCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "as_expression" | "type_assertion" => {
            let txt = node_text(node, ctx.source);
            let lhs = txt.split(" as ").next().unwrap_or(txt);
            if is_req_input_expr(lhs) || txt.contains("req.body") || txt.contains("req.query") {
                push_express(
                    out,
                    MernViolation::ExpressUnvalidatedRequestCast,
                    node,
                    String::from(
                        "TypeScript cast on `req.body`/`req.query` provides zero runtime safety; parse with Zod/Valibot",
                    ),
                );
            }
        }
        "variable_declarator" => {
            let destructured = !ctx.is_validated()
                && node
                    .child_by_field_name("name")
                    .is_some_and(|n| n.kind() == "object_pattern")
                && node
                    .child_by_field_name("value")
                    .is_some_and(|v| is_req_input_expr(node_text(v, ctx.source)));
            if destructured {
                push_express(
                    out,
                    MernViolation::ExpressUnvalidatedRequestCast,
                    node,
                    String::from(
                        "direct destructuring of unvalidated `req.body`/`req.query`/`req.params`; validate with a runtime schema first",
                    ),
                );
            }
        }
        "call_expression" => check_express_call(node, ctx, out),
        _ => {}
    }
}

fn walk_express(node: Node<'_>, ctx: ExpressCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    let next_ctx = if matches!(
        node.kind(),
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    ) {
        ctx.set_validated(has_validator_call(node_text(node, ctx.source)))
    } else {
        ctx
    };
    check_express_node(node, next_ctx, out);
    let mut cursor = node.walk();
    for ch in node.named_children(&mut cursor) {
        walk_express(ch, next_ctx, out);
    }
}

/// Extracts Express 5 & MERN API guardrails (`EXPRESS-*`) from a TS/JS CST.
pub fn extract_express_guardrails(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let has_auth = ["requireAuth", "authenticate", "jwt", "passport", "session"]
        .iter()
        .any(|m| source.contains(m));
    let flags = (u8::from(has_auth) * EF_AUTH)
        | (u8::from(is_app_module_with_routes(&doc.path, source)) * EF_APP_MOD);
    walk_express(
        root,
        ExpressCtx { source, flags },
        &mut doc.lang_rule_findings,
    );
}
