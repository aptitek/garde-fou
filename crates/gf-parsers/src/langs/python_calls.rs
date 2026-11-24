//! Tier 1 Python dangerous-call & `Code`-purity AST checks (`PY-S102`, `PY-S301`, `PY-S602`, `PY-T201`).

use gf_core::{
    DomainKind, EffectSighting, EffectSightingKind, LangRuleFinding, Span, UastDocument,
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

/// Structured specification for a Tier 1 Python AST finding (`GF080`).
pub struct PyFindingSpec {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
    pub remediation: &'static str,
}

pub fn push_py_finding(out: &mut Vec<LangRuleFinding>, spec: PyFindingSpec) {
    out.push(LangRuleFinding {
        code: String::from(spec.code),
        span: spec.span,
        message: spec.message,
        remediation: String::from(spec.remediation),
    });
}

pub fn compact_callee(call_node: Node<'_>, source: &str) -> String {
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

fn has_keyword_arg_eq(call_node: Node<'_>, source: &str, kw_name: &str, kw_val: &str) -> bool {
    let Some(args) = call_node.child_by_field_name("arguments") else {
        return false;
    };
    let mut cursor = args.walk();
    args.named_children(&mut cursor).any(|arg| {
        arg.kind() == "keyword_argument"
            && arg
                .child_by_field_name("name")
                .is_some_and(|n| node_text(n, source).trim() == kw_name)
            && arg
                .child_by_field_name("value")
                .is_some_and(|v| node_text(v, source).trim() == kw_val)
    })
}

fn has_safe_yaml_loader(call_node: Node<'_>, source: &str) -> bool {
    call_node
        .child_by_field_name("arguments")
        .is_some_and(|args| {
            let text = node_text(args, source);
            text.contains("SafeLoader") || text.contains("CSafeLoader")
        })
}

fn classify_security_call(node: Node<'_>, source: &str, callee: &str) -> Option<PyFindingSpec> {
    let span = node_span(node);
    if matches!(callee, "eval" | "exec" | "builtins.eval" | "builtins.exec") {
        return Some(PyFindingSpec {
            code: "PY-S102",
            span,
            message: format!(
                "dynamic code execution `{callee}(...)` enables arbitrary code execution"
            ),
            remediation: "Replace `eval`/`exec` with `ast.literal_eval`, explicit dispatch tables, or typed schema parsing.",
        });
    }
    if UNSAFE_DESERIALIZERS.contains(&callee)
        || (callee == "yaml.load" && !has_safe_yaml_loader(node, source))
    {
        return Some(PyFindingSpec {
            code: "PY-S301",
            span,
            message: format!(
                "unsafe deserialization `{callee}(...)` can execute untrusted payloads"
            ),
            remediation: "Use `json`, `pydantic`, or `yaml.safe_load(...)` instead of untrusted `pickle`/`yaml.load`.",
        });
    }
    if callee == "os.system"
        || (SUBPROCESS_CALLEES.contains(&callee)
            && has_keyword_arg_eq(node, source, "shell", "True"))
    {
        return Some(PyFindingSpec {
            code: "PY-S602",
            span,
            message: format!(
                "subprocess call `{callee}` with shell execution risks command injection"
            ),
            remediation: "Pass command arguments as a list (`subprocess.run([\"cmd\", arg], check=True)`) without `shell=True`.",
        });
    }
    None
}

pub fn inspect_py_call(node: Node<'_>, source: &str, domain: DomainKind, doc: &mut UastDocument) {
    let callee = compact_callee(node, source);
    let span = node_span(node);
    if matches!(callee.as_str(), "print" | "pprint" | "pprint.pprint") {
        doc.effect_sightings.push(EffectSighting {
            kind: EffectSightingKind::ConsoleOutput,
            detail: format!("{callee}(...)"),
            span,
        });
    }
    if let Some(spec) = classify_security_call(node, source, &callee) {
        push_py_finding(&mut doc.lang_rule_findings, spec);
    } else if domain == DomainKind::Code
        && matches!(callee.as_str(), "print" | "pprint" | "pprint.pprint")
    {
        push_py_finding(
            &mut doc.lang_rule_findings,
            PyFindingSpec {
                code: "PY-T201",
                span,
                message: format!(
                    "`{callee}(...)` console output is forbidden in `Code` domain modules"
                ),
                remediation: "Use `logging.getLogger(__name__)` or restrict console I/O to declared CLI `entrypoints`.",
            },
        );
    }
}
