//! Go call-site, `errcheck`, `context.Context`, and SQL-injection CST checks (`GO-*`).

use gf_core::{GoViolation, LangRuleFinding, SqlViolation, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

pub const GO_FLAG_IFACE_RET: u8 = 1 << 0;
pub const GO_FLAG_HAS_CTX: u8 = 1 << 1;
pub const GO_FLAG_IS_TEST: u8 = 1 << 2;
pub const GO_FLAG_IS_CMD: u8 = 1 << 3;

/// Context flags for inspecting a single Go function or method body.
#[derive(Clone, Copy)]
pub struct GoFnCtx<'a> {
    pub fn_name: &'a str,
    bits: u8,
}

impl<'a> GoFnCtx<'a> {
    pub const fn new(fn_name: &'a str, bits: u8) -> Self {
        Self { fn_name, bits }
    }

    pub const fn returns_iface(self) -> bool {
        (self.bits & GO_FLAG_IFACE_RET) != 0
    }

    pub const fn has_ctx_param(self) -> bool {
        (self.bits & GO_FLAG_HAS_CTX) != 0
    }

    pub const fn is_test_file(self) -> bool {
        (self.bits & GO_FLAG_IS_TEST) != 0
    }

    pub const fn is_cmd_or_main(self) -> bool {
        (self.bits & GO_FLAG_IS_CMD) != 0
    }
}

const CRITICAL_ERRCHECK_CALLS: &[&str] = &[
    "os.Remove",
    "os.RemoveAll",
    "os.WriteFile",
    ".Write(",
    ".Rollback(",
    ".Err(",
];

const SQL_EXEC_METHODS: &[&str] = &[
    "Query",
    "QueryContext",
    "QueryRow",
    "QueryRowContext",
    "Exec",
    "ExecContext",
    "Raw",
];

fn push_go(doc: &mut UastDocument, v: GoViolation, node: Node<'_>, msg: String) {
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, node_span(node), msg));
}

fn check_sql_call(call: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(func_n) = call.child_by_field_name("function") else {
        return;
    };
    let fn_txt = node_text(func_n, source);
    let method = fn_txt.rsplit('.').next().unwrap_or(fn_txt);
    if !SQL_EXEC_METHODS.contains(&method) {
        return;
    }
    let Some(args) = call.child_by_field_name("arguments") else {
        return;
    };
    let args_txt = node_text(args, source);
    let upper = args_txt.to_ascii_uppercase();
    let has_sql_kw = ["SELECT ", "INSERT ", "UPDATE ", "DELETE "]
        .iter()
        .any(|k| upper.contains(k));
    if has_sql_kw && (args_txt.contains("fmt.Sprintf(") || args_txt.contains("\" +")) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            SqlViolation::InjectionInterpolation,
            node_span(call),
            format!("dynamic SQL string construction in `{method}(...)`; use parameterized placeholders (`$1`/`?`)"),
        ));
    }
}

fn check_call_runtime_safety(node: Node<'_>, txt: &str, ctx: GoFnCtx<'_>, doc: &mut UastDocument) {
    let is_entry = ctx.is_test_file() || matches!(ctx.fn_name, "main" | "init");
    if txt.starts_with("panic(") && !is_entry {
        push_go(
            doc,
            GoViolation::GoroutineAndPanic,
            node,
            String::from("avoid `panic(...)` in library code; return a typed `error`"),
        );
    }
    if !ctx.is_cmd_or_main() && !ctx.is_test_file() && txt.starts_with("fmt.Print") {
        push_go(
            doc,
            GoViolation::GoroutineAndPanic,
            node,
            String::from("remove debug `fmt.Print*` from non-CLI package code"),
        );
    }
    if ctx.is_test_file() && txt.starts_with("time.Sleep(") {
        push_go(
            doc,
            GoViolation::GoroutineAndPanic,
            node,
            String::from(
                "avoid flaky `time.Sleep(...)` in Go tests; synchronize via channels or fake clocks",
            ),
        );
    }
}

pub fn check_call_node(node: Node<'_>, source: &str, ctx: GoFnCtx<'_>, doc: &mut UastDocument) {
    let txt = node_text(node, source);
    if ctx.has_ctx_param()
        && (txt.starts_with("context.Background(") || txt.starts_with("context.TODO("))
    {
        push_go(
            doc,
            GoViolation::ContextMisuse,
            node,
            String::from(
                "use the existing `ctx` parameter instead of `context.Background()` / `context.TODO()`",
            ),
        );
    }
    check_call_runtime_safety(node, txt, ctx, doc);
    check_sql_call(node, source, doc);
}

pub fn check_errcheck_stmt(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "short_var_declaration" | "assignment_statement" => {
            let Some(lhs) = node.child_by_field_name("left") else {
                return;
            };
            let Some(rhs) = node.child_by_field_name("right") else {
                return;
            };
            if !node_text(rhs, source).contains('(') {
                return;
            }
            let lhs_parts: Vec<&str> = node_text(lhs, source).split(',').map(str::trim).collect();
            if lhs_parts.last().copied() == Some("_") {
                push_go(
                    doc,
                    GoViolation::ErrcheckBlankOrIgnored,
                    node,
                    String::from("error return value discarded with `_` (`errcheck`)"),
                );
            }
        }
        "expression_statement" => {
            let txt = node_text(node, source).trim();
            if CRITICAL_ERRCHECK_CALLS.iter().any(|p| txt.contains(p)) {
                push_go(
                    doc,
                    GoViolation::ErrcheckBlankOrIgnored,
                    node,
                    format!("unchecked error return on critical call `{txt}` (`errcheck`)"),
                );
            }
        }
        _ => {}
    }
}
