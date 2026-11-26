//! Pure CST quirk & EF Core SQL-injection analyzers for C# (`CS-*` & `SQL-INJECTION-INTERPOLATION`).

use gf_core::{CSharpViolation, LangRuleFinding, SqlViolation, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const SAFE_EF_SQL_METHODS: &[&str] = &[
    "FromSql(",
    "FromSqlInterpolated(",
    "ExecuteSql(",
    "ExecuteSqlInterpolated(",
];

const UNSAFE_SQL_SINKS: &[&str] = &[
    "FromSqlRaw(",
    "ExecuteSqlRaw(",
    "ExecuteSqlRawAsync(",
    "SqlCommand(",
    ".Query(",
    ".QueryAsync(",
    ".QueryFirst(",
    ".QueryFirstAsync(",
    ".QuerySingle(",
    ".QuerySingleAsync(",
    ".Execute(",
    ".ExecuteAsync(",
];

pub(crate) const CS_FLAG_ASYNC: u8 = 1 << 0;
pub(crate) const CS_FLAG_TEST: u8 = 1 << 1;
pub(crate) const CS_FLAG_ENTRY: u8 = 1 << 2;

/// Compact bitmask context for C# method body inspection (obeys `max_struct_bools <= 2`).
#[derive(Clone, Copy, Default)]
pub(crate) struct CsWalkFlags(u8);

impl CsWalkFlags {
    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    const fn is_async(self) -> bool {
        (self.0 & CS_FLAG_ASYNC) != 0
    }

    const fn is_test(self) -> bool {
        (self.0 & CS_FLAG_TEST) != 0
    }

    const fn is_entry(self) -> bool {
        (self.0 & CS_FLAG_ENTRY) != 0
    }
}

fn push_cs(doc: &mut UastDocument, v: CSharpViolation, node: Node<'_>, msg: String) {
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, node_span(node), msg));
}

fn check_cs_catch(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let txt = node_text(node, source);
    let body_empty = txt
        .split_once('{')
        .and_then(|(_, rest)| rest.rsplit_once('}'))
        .is_some_and(|(inner, _)| {
            inner
                .lines()
                .all(|l| l.trim().is_empty() || l.trim().starts_with("//"))
        });
    if body_empty {
        push_cs(
            doc,
            CSharpViolation::RethrowDestroysStack,
            node,
            String::from("empty `catch` block silently swallows exception"),
        );
        return;
    }
    let Some(decl) = txt
        .split_once('(')
        .and_then(|(_, r)| r.split_once(')'))
        .map(|(d, _)| d)
    else {
        return;
    };
    let Some(ex_var) = decl.split_whitespace().last() else {
        return;
    };
    if !ex_var.is_empty() && txt.contains(&format!("throw {ex_var};")) {
        push_cs(
            doc,
            CSharpViolation::RethrowDestroysStack,
            node,
            format!(
                "`throw {ex_var};` resets the original stack trace (`CA2200`); use bare `throw;`"
            ),
        );
    }
}

fn check_cs_using_unawaited(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let txt = node_text(node, source);
    let header = txt.lines().next().unwrap_or(txt);
    let is_using_decl = node.kind() == "using_statement"
        || (node.kind() == "local_declaration_statement"
            && header.trim_start().starts_with("using "));
    if is_using_decl && header.contains("Async(") && !header.contains("await ") {
        push_cs(
            doc,
            CSharpViolation::DisposeAndCancellationSafety,
            node,
            String::from(
                "`using` disposes an unawaited `Task` instead of the resource (`MA0129`); `await` before `using`",
            ),
        );
    }
}

fn check_cs_sql_call(node: Node<'_>, txt: &str, doc: &mut UastDocument) {
    if SAFE_EF_SQL_METHODS.iter().any(|s| txt.contains(s)) {
        return;
    }
    if !UNSAFE_SQL_SINKS.iter().any(|s| txt.contains(s)) {
        return;
    }
    let upper = txt.to_ascii_uppercase();
    let has_sql = upper.contains("SELECT ")
        || upper.contains("INSERT ")
        || upper.contains("UPDATE ")
        || upper.contains("DELETE ");
    let dynamic = txt.contains("$\"") || txt.contains("string.Format(") || txt.contains("\" +");
    if has_sql && dynamic {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            SqlViolation::InjectionInterpolation,
            node_span(node),
            String::from("dynamic SQL interpolation in raw/Dapper/SqlCommand call; use EF Core `FromSql($\"...\")` or parameterized `@p0`"),
        ));
    }
}

fn check_cs_runtime_invocation(
    node: Node<'_>,
    txt: &str,
    flags: CsWalkFlags,
    doc: &mut UastDocument,
) {
    if txt.starts_with("GC.Collect(") {
        push_cs(
            doc,
            CSharpViolation::RethrowDestroysStack,
            node,
            String::from("avoid explicit `GC.Collect()` calls in application code"),
        );
    }
    if flags.is_async() && txt.starts_with("Thread.Sleep(") {
        push_cs(
            doc,
            CSharpViolation::RethrowDestroysStack,
            node,
            String::from(
                "`Thread.Sleep(...)` inside `async` method blocks the thread pool; use `await Task.Delay(...)`",
            ),
        );
    }
    if !flags.is_test() && !flags.is_entry() && txt.starts_with("Console.Write") {
        push_cs(
            doc,
            CSharpViolation::RethrowDestroysStack,
            node,
            String::from("remove `Console.Write*` from library code; inject `ILogger`"),
        );
    }
}

fn check_cs_invocation_or_member(
    node: Node<'_>,
    source: &str,
    flags: CsWalkFlags,
    doc: &mut UastDocument,
) {
    let txt = node_text(node, source).trim();
    if node.kind() == "member_access_expression" && txt.ends_with(".Result") {
        push_cs(
            doc,
            CSharpViolation::SyncOverAsync,
            node,
            String::from("blocking `.Result` causes thread-pool starvation/deadlocks; use `await`"),
        );
    }
    if node.kind() != "invocation_expression" {
        return;
    }
    if txt.ends_with(".Wait()") || txt.contains(".GetAwaiter().GetResult()") {
        push_cs(
            doc,
            CSharpViolation::SyncOverAsync,
            node,
            String::from(
                "blocking `.Wait()` / `.GetAwaiter().GetResult()` causes deadlocks; use `await`",
            ),
        );
    }
    check_cs_runtime_invocation(node, txt, flags, doc);
    check_cs_sql_call(node, txt, doc);
}

/// Walks a C# method/local-function body for `CS-*` quirks and EF Core SQL injection.
pub(crate) fn walk_cs_body(
    node: Node<'_>,
    source: &str,
    flags: CsWalkFlags,
    doc: &mut UastDocument,
) {
    match node.kind() {
        "catch_clause" => check_cs_catch(node, source, doc),
        "using_statement" | "local_declaration_statement" => {
            check_cs_using_unawaited(node, source, doc);
        }
        "invocation_expression" | "member_access_expression" => {
            check_cs_invocation_or_member(node, source, flags, doc);
        }
        _ => {}
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        walk_cs_body(ch, source, flags, doc);
    }
}
