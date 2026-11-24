//! Tier 1 Python dangerous-call, `typing.cast(Any, ...)`, and debug/console AST checks
//! (`PY-S102`, `PY-S301`, `PY-S506`, `PY-S602`, `PY-ANN401`, `PY-T100`, `PY-T201`).

use std::collections::BTreeSet;

use gf_core::{
    DebugArtifactKind, DomainKind, EffectSighting, EffectSightingKind, ImportDecl, LangRuleFinding,
    PythonViolation, UastDocument, UnsafeExecKind,
};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const UNSAFE_DESERIALIZERS: &[&str] = &[
    "pickle.loads",
    "pickle.load",
    "_pickle.loads",
    "_pickle.load",
    "cPickle.loads",
    "cPickle.load",
    "marshal.loads",
    "marshal.load",
    "shelve.open",
];

const SUBPROCESS_CALLEES: &[&str] = &[
    "subprocess.run",
    "subprocess.Popen",
    "subprocess.call",
    "subprocess.check_call",
    "subprocess.check_output",
    "run",
    "Popen",
    "call",
    "check_call",
    "check_output",
];

const DEBUGGER_CALLEES: &[&str] = &[
    "breakpoint",
    "builtins.breakpoint",
    "pdb.set_trace",
    "ipdb.set_trace",
    "pudb.set_trace",
];

const CONSOLE_CALLEES: &[&str] = &["print", "builtins.print", "pprint", "pprint.pprint"];

/// Shared context passed across Tier 1 Python guardrail call inspections.
#[derive(Clone, Copy)]
pub(super) struct PyCallCtx<'a> {
    pub source: &'a str,
    pub domain: DomainKind,
    pub bare_unsafe: &'a BTreeSet<String>,
}

fn collect_bare_from_import(imp: &ImportDecl, out: &mut BTreeSet<String>) {
    let mod_name = imp.module_source.trim();
    let allowed: &[&str] = if matches!(mod_name, "pickle" | "_pickle" | "cPickle" | "marshal") {
        &["load", "loads"]
    } else if matches!(mod_name, "pdb" | "ipdb" | "pudb") {
        &["set_trace"]
    } else {
        return;
    };
    for sym in &imp.symbols {
        if allowed.contains(&sym.as_str()) {
            out.insert(sym.clone());
        }
    }
}

/// Collects bare unsafe function symbols imported via `from pickle/marshal/pdb import ...`.
pub(super) fn collect_bare_unsafe_imports(doc: &UastDocument) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for imp in &doc.imports {
        collect_bare_from_import(imp, &mut set);
    }
    set
}

/// Compacts a Python call callee expression by stripping internal whitespace.
pub(super) fn compact_callee(call_node: Node<'_>, source: &str) -> String {
    call_node
        .child_by_field_name("function")
        .map(|fn_node| {
            node_text(fn_node, source)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect()
        })
        .unwrap_or_default()
}

fn find_keyword_value<'a>(call_node: Node<'a>, source: &str, kw_name: &str) -> Option<Node<'a>> {
    let args = call_node.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    args.named_children(&mut cursor).find_map(|arg| {
        (arg.kind() == "keyword_argument"
            && arg
                .child_by_field_name("name")
                .is_some_and(|n| node_text(n, source).trim() == kw_name))
        .then(|| arg.child_by_field_name("value"))
        .flatten()
    })
}

fn has_keyword_arg_eq(call_node: Node<'_>, source: &str, kw_name: &str, kw_val: &str) -> bool {
    find_keyword_value(call_node, source, kw_name)
        .is_some_and(|val| node_text(val, source).trim() == kw_val)
}

fn has_safe_yaml_loader(call_node: Node<'_>, source: &str) -> bool {
    let Some(loader_val) = find_keyword_value(call_node, source, "Loader") else {
        return false;
    };
    if !matches!(loader_val.kind(), "identifier" | "attribute") {
        return false;
    }
    let leaf = node_text(loader_val, source)
        .trim()
        .rsplit('.')
        .next()
        .unwrap_or("");
    matches!(leaf, "SafeLoader" | "CSafeLoader")
}

fn inspect_cast_any_arg(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if node.kind() == "identifier" && node_text(node, source) == "Any" {
        out.push(LangRuleFinding::new(
            PythonViolation::ForbiddenAny,
            node_span(node),
            "`typing.cast(Any, ...)` bypasses static typing in `Code`",
        ));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        inspect_cast_any_arg(child, source, out);
    }
}

fn classify_unsafe_exec(
    node: Node<'_>,
    callee: &str,
    ctx: PyCallCtx<'_>,
) -> Option<(UnsafeExecKind, String)> {
    if matches!(callee, "eval" | "exec" | "builtins.eval" | "builtins.exec") {
        return Some((
            UnsafeExecKind::DynamicEval,
            format!("dynamic code execution `{callee}(...)` enables arbitrary code execution"),
        ));
    }
    if UNSAFE_DESERIALIZERS.contains(&callee)
        || (matches!(callee, "load" | "loads") && ctx.bare_unsafe.contains(callee))
    {
        return Some((
            UnsafeExecKind::UnsafePickle,
            format!("unsafe deserialization `{callee}(...)` can execute untrusted payloads"),
        ));
    }
    if matches!(callee, "yaml.load" | "yaml.load_all") && !has_safe_yaml_loader(node, ctx.source) {
        return Some((
            UnsafeExecKind::UnsafeYamlLoad,
            format!(
                "`{callee}(...)` without `Loader=yaml.SafeLoader` risks arbitrary object instantiation"
            ),
        ));
    }
    if matches!(callee, "os.system" | "os.popen")
        || (SUBPROCESS_CALLEES.contains(&callee)
            && has_keyword_arg_eq(node, ctx.source, "shell", "True"))
    {
        return Some((
            UnsafeExecKind::ShellSubprocess,
            format!("subprocess call `{callee}` with shell execution risks command injection"),
        ));
    }
    None
}

fn classify_debug_call(callee: &str, ctx: PyCallCtx<'_>) -> Option<(DebugArtifactKind, String)> {
    if ctx.domain != DomainKind::Code {
        return None;
    }
    if DEBUGGER_CALLEES.contains(&callee)
        || (callee == "set_trace" && ctx.bare_unsafe.contains("set_trace"))
    {
        return Some((
            DebugArtifactKind::DebuggerTrace,
            format!("leftover debugger hook `{callee}()` in `Code` module"),
        ));
    }
    CONSOLE_CALLEES.contains(&callee).then(|| {
        (
            DebugArtifactKind::PrintCall,
            format!("`{callee}(...)` console output is forbidden in `Code` domain modules"),
        )
    })
}

/// Inspects a Python `call` node for `GF080` security/debug findings and console `EffectSighting`s.
pub(super) fn inspect_py_call(node: Node<'_>, ctx: PyCallCtx<'_>, doc: &mut UastDocument) {
    let callee = compact_callee(node, ctx.source);
    let span = node_span(node);
    if CONSOLE_CALLEES.contains(&callee.as_str()) {
        doc.effect_sightings.push(EffectSighting {
            kind: EffectSightingKind::ConsoleOutput,
            detail: format!("{callee}(...)"),
            span,
        });
    }
    if ctx.domain == DomainKind::Code
        && matches!(
            callee.as_str(),
            "cast" | "typing.cast" | "typing_extensions.cast"
        )
        && let Some(first_arg) = node
            .child_by_field_name("arguments")
            .and_then(|a| a.named_child(0))
    {
        inspect_cast_any_arg(first_arg, ctx.source, &mut doc.lang_rule_findings);
    }

    if let Some((exec_kind, message)) = classify_unsafe_exec(node, &callee, ctx) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            PythonViolation::UnsafeExecution(exec_kind),
            span,
            message,
        ));
    } else if let Some((dbg_kind, message)) = classify_debug_call(&callee, ctx) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            PythonViolation::LeftoverDebugArtifact(dbg_kind),
            span,
            message,
        ));
    }
}
