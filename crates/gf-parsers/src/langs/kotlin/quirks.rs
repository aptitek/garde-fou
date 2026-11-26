//! Pure CST quirk & SQL-injection analyzers for Kotlin (`KT-*` & `SQL-INJECTION-INTERPOLATION`).

use gf_core::{BypassFinding, KotlinViolation, LangRuleFinding, SqlViolation, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const COROUTINE_BUILDERS: &[&str] = &["launch", "async", "withContext", "flow"];
const SQL_SINKS: &[&str] = &[
    "executeQuery",
    "prepareStatement",
    "jdbcTemplate.query",
    ".exec(",
];

pub(crate) const KT_FLAG_SUSPEND: u8 = 1 << 0;
const FLAG_IN_LOOP: u8 = 1 << 1;
pub(crate) const KT_FLAG_IS_TEST: u8 = 1 << 2;
pub(crate) const KT_FLAG_IS_MAIN_OR_KTS: u8 = 1 << 3;

/// Compact bitmask context for Kotlin CST traversal (obeys `max_struct_bools <= 2`).
#[derive(Clone, Copy, Default)]
pub(crate) struct KtWalkFlags(u8);

impl KtWalkFlags {
    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    const fn is_suspend(self) -> bool {
        (self.0 & KT_FLAG_SUSPEND) != 0
    }

    const fn in_loop(self) -> bool {
        (self.0 & FLAG_IN_LOOP) != 0
    }

    const fn is_test(self) -> bool {
        (self.0 & KT_FLAG_IS_TEST) != 0
    }

    const fn is_main_or_kts(self) -> bool {
        (self.0 & KT_FLAG_IS_MAIN_OR_KTS) != 0
    }

    const fn with_suspend(self) -> Self {
        Self(self.0 | KT_FLAG_SUSPEND)
    }

    const fn with_loop(self) -> Self {
        Self(self.0 | FLAG_IN_LOOP)
    }
}

fn push_kt(doc: &mut UastDocument, v: KotlinViolation, node: Node<'_>, msg: String) {
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, node_span(node), msg));
}

pub(crate) fn inspect_kt_bypasses(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let txt = node_text(n, source).trim();
        let is_bypass = (n.kind().contains("comment")
            && (txt.contains("ktlint-disable") || txt.contains("detekt-disable")))
            || (n.kind() == "annotation" && txt.starts_with("@Suppress("));
        if is_bypass {
            doc.bypass_findings.push(BypassFinding {
                directive: String::from(txt),
                span: node_span(n),
            });
        }
        let mut c = n.walk();
        for ch in n.children(&mut c) {
            stack.push(ch);
        }
    }
}

fn check_kt_catch(node: Node<'_>, source: &str, flags: KtWalkFlags, doc: &mut UastDocument) {
    let txt = node_text(node, source);
    let body_is_empty = txt
        .split_once('{')
        .and_then(|(_, rest)| rest.rsplit_once('}'))
        .is_some_and(|(inner, _)| {
            inner
                .lines()
                .all(|l| l.trim().is_empty() || l.trim().starts_with("//"))
        });
    if body_is_empty {
        push_kt(
            doc,
            KotlinViolation::SwallowedExceptionAndAny,
            node,
            String::from("empty `catch` block silently swallows exception"),
        );
        return;
    }
    if flags.is_suspend() {
        let broad = txt.contains(": Exception") || txt.contains(": Throwable");
        let rethrows_cancel = (txt.contains("CancellationException") && txt.contains("throw"))
            || txt.contains("ensureActive()");
        if broad && !rethrows_cancel {
            push_kt(
                doc,
                KotlinViolation::SuspendRunCatchingCancellation,
                node,
                String::from(
                    "broad `catch` in `suspend` function swallows `CancellationException`",
                ),
            );
        }
    }
}

fn check_kt_coroutine_call(node: Node<'_>, txt: &str, flags: KtWalkFlags, doc: &mut UastDocument) {
    if flags.is_suspend() && txt.starts_with("runCatching") {
        push_kt(
            doc,
            KotlinViolation::SuspendRunCatchingCancellation,
            node,
            String::from(
                "`runCatching` inside `suspend`/coroutine swallows `CancellationException`",
            ),
        );
    }
    if txt.starts_with("GlobalScope.launch") || txt.starts_with("GlobalScope.async") {
        push_kt(
            doc,
            KotlinViolation::CoroutineStructuredSafety,
            node,
            String::from("`GlobalScope` breaks structured concurrency; inject a `CoroutineScope`"),
        );
    }
    if flags.is_suspend() && txt.starts_with("Thread.sleep(") {
        push_kt(
            doc,
            KotlinViolation::CoroutineStructuredSafety,
            node,
            String::from(
                "`Thread.sleep(...)` inside `suspend`/coroutine blocks the dispatcher thread; use `delay(...)`",
            ),
        );
    }
}

fn check_kt_call(node: Node<'_>, source: &str, flags: KtWalkFlags, doc: &mut UastDocument) {
    let txt = node_text(node, source).trim();
    check_kt_coroutine_call(node, txt, flags, doc);
    let prod_lib = !flags.is_test() && !flags.is_main_or_kts();
    if prod_lib && txt.starts_with("runBlocking") {
        push_kt(
            doc,
            KotlinViolation::CoroutineStructuredSafety,
            node,
            String::from("avoid `runBlocking` in library code; propagate `suspend`"),
        );
    }
    if prod_lib && (txt.starts_with("println(") || txt.starts_with("print(")) {
        push_kt(
            doc,
            KotlinViolation::SwallowedExceptionAndAny,
            node,
            String::from("remove debug `println`/`print` from production Kotlin code"),
        );
    }
    check_kt_sql_call(node, txt, doc);
}

fn check_kt_sql_call(node: Node<'_>, txt: &str, doc: &mut UastDocument) {
    if !SQL_SINKS.iter().any(|s| txt.contains(s)) {
        return;
    }
    let upper = txt.to_ascii_uppercase();
    let has_sql = ["SELECT ", "INSERT ", "UPDATE ", "DELETE "]
        .iter()
        .any(|k| upper.contains(k));
    if has_sql && (txt.contains('$') || txt.contains("\" +")) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            SqlViolation::InjectionInterpolation,
            node_span(node),
            String::from("interpolated SQL template string in Kotlin database call; use parameterized `?` bindings"),
        ));
    }
}

fn inspect_kt_node(node: Node<'_>, source: &str, flags: KtWalkFlags, doc: &mut UastDocument) {
    match node.kind() {
        "not_null_assertion" | "!!" if !flags.is_test() => push_kt(
            doc,
            KotlinViolation::NoNotNullAssertion,
            node,
            String::from(
                "non-null assertion `!!` is forbidden in `Code`; use `?.`, `?:`, or `requireNotNull`",
            ),
        ),
        "catch_block" => check_kt_catch(node, source, flags, doc),
        "call_expression" | "navigation_expression" => check_kt_call(node, source, flags, doc),
        "spread_expression" if flags.in_loop() => push_kt(
            doc,
            KotlinViolation::SwallowedExceptionAndAny,
            node,
            String::from("avoid `*spread` operator inside loops (`SpreadOperator`)"),
        ),
        _ => {}
    }
}

fn next_kt_flags(node: Node<'_>, source: &str, flags: KtWalkFlags) -> KtWalkFlags {
    let kind = node.kind();
    let mut next = flags;
    if matches!(
        kind,
        "for_statement" | "while_statement" | "do_while_statement"
    ) {
        next = next.with_loop();
    }
    if kind == "call_expression" {
        let head = node_text(node, source).trim_start();
        if COROUTINE_BUILDERS.iter().any(|b| head.starts_with(b)) {
            next = next.with_suspend();
        }
    }
    next
}

/// Recursively inspects a Kotlin function/class body for `KT-*` quirks and SQL injection.
pub(crate) fn walk_kt_body(
    node: Node<'_>,
    source: &str,
    flags: KtWalkFlags,
    doc: &mut UastDocument,
) {
    inspect_kt_node(node, source, flags, doc);
    let next = next_kt_flags(node, source, flags);
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        if ch.is_named() || ch.kind() == "!!" {
            walk_kt_body(ch, source, next, doc);
        }
    }
}
