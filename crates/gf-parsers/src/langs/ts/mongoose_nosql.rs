//! MongoDB & Mongoose TypeScript security and performance CST guardrails (`MONGO-*` under `GF080`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, MernViolation, UastDocument};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

#[rustfmt::skip]
const MONGO_FILTER_METHODS: &[&str] = &[
    "find", "findOne", "findOneAndUpdate", "findOneAndDelete", "updateOne", "updateMany",
    "deleteOne", "deleteMany", "replaceOne", "countDocuments", "distinct",
];

const MONGO_LOOP_METHODS: &[&str] = &["find", "findOne", "findById", "updateOne", "create", "save"];

fn push_mongo(out: &mut Vec<LangRuleFinding>, v: MernViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn extract_tainted_ident(n: Node<'_>, source: &str) -> Option<String> {
    if n.kind() != "variable_declarator" {
        return None;
    }
    let nm = n.child_by_field_name("name")?;
    let val = n.child_by_field_name("value")?;
    let vtxt = node_text(val, source);
    let is_req =
        vtxt.contains("req.body") || vtxt.contains("req.query") || vtxt.contains("req.params");
    let is_cast = matches!(val.kind(), "as_expression" | "type_assertion");
    if !(is_req && (is_cast || vtxt.trim().starts_with("req."))) {
        return None;
    }
    let ident = node_text(nm, source).trim();
    ident
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        .then(|| String::from(ident))
}

fn collect_unvalidated_cast_vars(root: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if let Some(ident) = extract_tainted_ident(n, source) {
            vars.insert(ident);
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
    vars
}

fn arg_has_tainted_input(arg_txt: &str, tainted_vars: &BTreeSet<String>) -> bool {
    if arg_txt.contains("$eq") {
        return false;
    }
    if arg_txt.contains("req.body")
        || arg_txt.contains("req.query")
        || arg_txt.contains("req.params")
    {
        return true;
    }
    tainted_vars.iter().any(|v| {
        arg_txt.contains(&format!("{v}."))
            || arg_txt
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|tok| tok == v)
    })
}

fn first_arg_text<'a>(call: Node<'_>, source: &'a str) -> Option<&'a str> {
    let args = call.child_by_field_name("arguments")?;
    let mut c = args.walk();
    args.named_children(&mut c)
        .next()
        .map(|n| node_text(n, source).trim())
}

const MF_IN_LOOP: u8 = 1 << 0;
const MF_HAS_INDEX: u8 = 1 << 1;

#[derive(Clone, Copy)]
struct MongoCtx<'a> {
    source: &'a str,
    tainted: &'a BTreeSet<String>,
    flags: u8,
}

impl MongoCtx<'_> {
    const fn in_loop(self) -> bool {
        (self.flags & MF_IN_LOOP) != 0
    }
    const fn file_has_index(self) -> bool {
        (self.flags & MF_HAS_INDEX) != 0
    }
    const fn with_loop(self) -> Self {
        Self {
            source: self.source,
            tainted: self.tainted,
            flags: self.flags | MF_IN_LOOP,
        }
    }
}

fn check_mongo_filter_arg(
    node: Node<'_>,
    method: &str,
    ctx: MongoCtx<'_>,
    out: &mut Vec<LangRuleFinding>,
) {
    let Some(first_arg) = first_arg_text(node, ctx.source) else {
        return;
    };
    if arg_has_tainted_input(first_arg, ctx.tainted) {
        push_mongo(
            out,
            MernViolation::MongoNoSqlInjection,
            node,
            format!(
                "unvalidated request input passed to MongoDB `{method}(...)` filter; validate with Zod/Valibot and wrap with `{{ $eq: ... }}`"
            ),
        );
    }
    let compact: String = first_arg.chars().filter(|c| !c.is_whitespace()).collect();
    if matches!(method, "deleteMany" | "updateMany") && compact == "{}" {
        push_mongo(
            out,
            MernViolation::MongoUnboundedQueryOrMutation,
            node,
            format!("unbounded `{method}({{}})` mutates or deletes the entire collection"),
        );
    }
}

fn check_mongo_call(node: Node<'_>, ctx: MongoCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, ctx.source).trim();
    let Some(method) = ftxt.rsplit('.').next() else {
        return;
    };
    if !ftxt.contains('.') {
        return;
    }
    if MONGO_FILTER_METHODS.contains(&method) {
        check_mongo_filter_arg(node, method, ctx, out);
    }
    let is_awaited = node
        .parent()
        .is_some_and(|p| p.kind() == "await_expression");
    if method == "find" && is_awaited && !node_text(node, ctx.source).contains(".limit(") {
        push_mongo(
            out,
            MernViolation::MongoUnboundedQueryOrMutation,
            node,
            String::from("unbounded `await Model.find(...)` without `.limit(...)`"),
        );
    }
    if ctx.in_loop() && is_awaited && MONGO_LOOP_METHODS.contains(&method) {
        push_mongo(
            out,
            MernViolation::MongoUnboundedQueryOrMutation,
            node,
            format!(
                "awaited MongoDB `{method}(...)` inside loop causes N+1 queries; use `$in` or `bulkWrite`"
            ),
        );
    }
}

fn check_schema_pair(node: Node<'_>, ctx: MongoCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    let Some(key) = node.child_by_field_name("key") else {
        return;
    };
    let ktxt = node_text(key, ctx.source).trim().trim_matches(['"', '\'']);
    if matches!(ktxt, "$where" | "$function" | "$accumulator") {
        push_mongo(
            out,
            MernViolation::MongoNoSqlInjection,
            node,
            format!("server-side JS MongoDB operator `{ktxt}` is forbidden"),
        );
    }
    if let Some(val) = node.child_by_field_name("value")
        && val.kind() == "object"
    {
        let vtxt = node_text(val, ctx.source);
        if vtxt.contains("ref:") && !vtxt.contains("index:") && !ctx.file_has_index() {
            push_mongo(
                out,
                MernViolation::MongoSchemaStrictness,
                node,
                format!("Mongoose `ref` field `{ktxt}` is missing `index: true`"),
            );
        }
    }
}

fn walk_mongo(node: Node<'_>, ctx: MongoCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "call_expression" => check_mongo_call(node, ctx, out),
        "pair" => check_schema_pair(node, ctx, out),
        "member_expression" => {
            let txt = node_text(node, ctx.source).trim();
            if txt == "Schema.Types.Mixed" || txt == "SchemaTypes.Mixed" {
                push_mongo(
                    out,
                    MernViolation::MongoSchemaStrictness,
                    node,
                    String::from(
                        "avoid untyped `Schema.Types.Mixed`; declare an explicit sub-schema",
                    ),
                );
            }
        }
        _ => {}
    }
    let next_ctx = if matches!(
        node.kind(),
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
    ) {
        ctx.with_loop()
    } else {
        ctx
    };
    let mut cursor = node.walk();
    for ch in node.named_children(&mut cursor) {
        walk_mongo(ch, next_ctx, out);
    }
}

/// Extracts MongoDB & Mongoose TypeScript guardrails (`MONGO-*`) from a TS/JS CST.
pub fn extract_mongoose_nosql_rules(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let tainted = collect_unvalidated_cast_vars(root, source);
    let flags = u8::from(source.contains(".index(")) * MF_HAS_INDEX;
    let ctx = MongoCtx {
        source,
        tainted: &tainted,
        flags,
    };
    walk_mongo(root, ctx, &mut doc.lang_rule_findings);
}
