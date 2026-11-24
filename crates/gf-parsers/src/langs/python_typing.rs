//! Python decorator classification, method receiver arity (`self`/`cls`),
//! and public callable type-annotation verification (`PY-ANN201`).

use gf_core::{LangRuleFinding, PythonViolation};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text, single_param_arity};

const PY_TUPLE_KINDS: &[&str] = &["tuple_pattern", "tuple"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PyMethodReceiver {
    None,
    SelfReceiver,
    ClsReceiver,
}

fn clean_decorator_head(raw: &str) -> &str {
    let without_at = raw.trim().trim_start_matches('@').trim();
    without_at.split('(').next().unwrap_or(without_at).trim()
}

fn collect_decorator_heads<'a>(def_node: Node<'_>, source: &'a str) -> Vec<&'a str> {
    let Some(parent) = def_node
        .parent()
        .filter(|p| p.kind() == "decorated_definition")
    else {
        return Vec::new();
    };
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .filter(|c| c.kind() == "decorator")
        .map(|dec| clean_decorator_head(node_text(dec, source)))
        .filter(|h| !h.is_empty())
        .collect()
}

/// Returns `true` when `def_node` is decorated with `@dataclass` / `@dataclasses.dataclass`.
pub fn has_dataclass_decorator(def_node: Node<'_>, source: &str) -> bool {
    collect_decorator_heads(def_node, source)
        .into_iter()
        .any(|h| h.rsplit('.').next() == Some("dataclass"))
}

/// Returns `true` when `def_node` is decorated with `@pytest.fixture` or `@fixture`.
pub fn has_pytest_fixture_decorator(def_node: Node<'_>, source: &str) -> bool {
    collect_decorator_heads(def_node, source)
        .into_iter()
        .any(|h| h == "fixture" || h.ends_with(".fixture"))
}

fn classify_py_receiver(
    fn_node: Node<'_>,
    fn_name: &str,
    in_class: bool,
    source: &str,
) -> PyMethodReceiver {
    if !in_class {
        return PyMethodReceiver::None;
    }
    let heads = collect_decorator_heads(fn_node, source);
    if heads
        .iter()
        .any(|h| h.rsplit('.').next() == Some("staticmethod"))
    {
        return PyMethodReceiver::None;
    }
    if heads
        .iter()
        .any(|h| h.rsplit('.').next() == Some("classmethod"))
        || matches!(fn_name, "__init_subclass__" | "__class_getitem__")
    {
        return PyMethodReceiver::ClsReceiver;
    }
    PyMethodReceiver::SelfReceiver
}

fn collect_effective_param_nodes(params_node: Option<Node<'_>>) -> Vec<Node<'_>> {
    let Some(node) = params_node else {
        return Vec::new();
    };
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| {
            !matches!(
                c.kind(),
                "comment" | "positional_separator" | "keyword_separator"
            )
        })
        .collect()
}

fn param_base_ident<'a>(param: Node<'_>, source: &'a str) -> &'a str {
    let raw = node_text(param, source).trim();
    let before_colon = raw.split(':').next().unwrap_or(raw);
    let before_eq = before_colon.split('=').next().unwrap_or(before_colon);
    before_eq.trim().trim_start_matches('*').trim()
}

fn is_implicit_receiver_param(param: Node<'_>, source: &str, receiver: PyMethodReceiver) -> bool {
    let ident = param_base_ident(param, source);
    (receiver == PyMethodReceiver::SelfReceiver && ident == "self")
        || (receiver == PyMethodReceiver::ClsReceiver && ident == "cls")
}

/// Counts Python callable parameters while skipping implicit `self`/`cls` receivers (`GF003`).
pub(super) fn count_callable_py_params(
    fn_node: Node<'_>,
    fn_name: &str,
    in_class: bool,
    source: &str,
) -> u32 {
    let receiver = classify_py_receiver(fn_node, fn_name, in_class, source);
    let params = collect_effective_param_nodes(fn_node.child_by_field_name("parameters"));
    let skip = usize::from(
        params
            .first()
            .is_some_and(|first| is_implicit_receiver_param(*first, source, receiver)),
    );
    params
        .into_iter()
        .skip(skip)
        .map(|p| single_param_arity(p, Some(source), PY_TUPLE_KINDS))
        .fold(0_u32, u32::saturating_add)
}

fn is_typed_param_node(param: Node<'_>) -> bool {
    matches!(param.kind(), "typed_parameter" | "typed_default_parameter")
}

/// Checks public functions and methods in `DomainKind::Code` for missing parameter
/// or return type annotations (`PY-ANN201`).
pub fn inspect_public_fn_annotations(
    fn_node: Node<'_>,
    source: &str,
    in_class: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let fn_name = field_name_string(fn_node, "name", "", source);
    if fn_name.is_empty() || fn_name.starts_with('_') {
        return;
    }
    let receiver = classify_py_receiver(fn_node, &fn_name, in_class, source);
    let params = collect_effective_param_nodes(fn_node.child_by_field_name("parameters"));

    for (idx, param) in params.into_iter().enumerate() {
        if idx == 0 && is_implicit_receiver_param(param, source, receiver) {
            continue;
        }
        if !is_typed_param_node(param) {
            let p_label = node_text(param, source)
                .split('=')
                .next()
                .unwrap_or("<param>")
                .trim();
            out.push(LangRuleFinding {
                violation: PythonViolation::MissingPublicTypeAnnotation,
                span: node_span(param),
                message: format!(
                    "public callable `{fn_name}` is missing a type annotation on parameter `{p_label}`"
                ),
            });
        }
    }

    if fn_node.child_by_field_name("return_type").is_none() {
        let header_span = fn_node
            .child_by_field_name("name")
            .map_or_else(|| node_span(fn_node), node_span);
        out.push(LangRuleFinding {
            violation: PythonViolation::MissingPublicTypeAnnotation,
            span: header_span,
            message: format!(
                "public callable `{fn_name}` is missing a return type annotation (`-> ...`)"
            ),
        });
    }
}
