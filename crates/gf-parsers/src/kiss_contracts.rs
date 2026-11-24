//! Behavioral contract (`trait` / `interface` / `type` object / `ABC` / `Protocol`)
//! and concrete implementation extractor (`GF075` YAGNI single-implementation check).

mod ts_contracts;

use gf_core::{ContractDecl, ContractImpl, Language, UastDocument};
use tree_sitter::Node;

use crate::cst::{
    count_children_of_kind, count_named_kinds, field_name_string, node_span, node_text,
};
use crate::langs::python_typing::has_dataclass_decorator;
use ts_contracts::{has_pub_visibility, normalize_contract_ident, record_ts_contracts};

const PY_ABSTRACT_BASES: &[&str] = &["ABC", "ABCMeta", "Protocol"];
const PY_DATA_OR_IGNORED_BASES: &[&str] = &[
    "ABC",
    "ABCMeta",
    "Protocol",
    "object",
    "Exception",
    "BaseException",
    "Enum",
    "IntEnum",
    "StrEnum",
    "Flag",
    "IntFlag",
    "BaseModel",
    "TypedDict",
    "NamedTuple",
    "Generic",
];

fn record_rust_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if node.kind() == "trait_item" {
        let methods = count_children_of_kind(
            node.child_by_field_name("body"),
            &["function_item", "function_signature_item"],
        );
        if methods >= 1 {
            doc.contracts.push(ContractDecl {
                name: field_name_string(node, "name", "<trait>", source),
                method_count: methods,
                is_public: has_pub_visibility(node, source),
                span: node_span(node),
            });
        }
    } else if node.kind() == "impl_item"
        && let Some(tr_node) = node.child_by_field_name("trait")
        && let Some(ty_node) = node.child_by_field_name("type")
    {
        let contract = normalize_contract_ident(node_text(tr_node, source));
        let implementor = node_text(ty_node, source).trim();
        if !contract.is_empty() && !implementor.is_empty() {
            doc.contract_impls.insert(ContractImpl {
                contract: String::from(contract),
                implementor: String::from(implementor),
            });
        }
    }
}

fn extract_py_base_ident<'a>(arg: Node<'_>, source: &'a str) -> Option<&'a str> {
    if arg.kind() == "keyword_argument" {
        let is_meta = arg
            .child_by_field_name("name")
            .is_some_and(|n| node_text(n, source).trim() == "metaclass");
        let val = arg.child_by_field_name("value")?;
        return is_meta
            .then(|| normalize_contract_ident(node_text(val, source)))
            .filter(|s| !s.is_empty());
    }
    let norm = normalize_contract_ident(node_text(arg, source));
    (!norm.is_empty()).then_some(norm)
}

fn collect_py_superclasses(node: Node<'_>, source: &str) -> Vec<String> {
    let Some(sup) = node.child_by_field_name("superclasses") else {
        return Vec::new();
    };
    let mut c = sup.walk();
    sup.named_children(&mut c)
        .filter_map(|arg| extract_py_base_ident(arg, source))
        .map(String::from)
        .collect()
}

fn record_py_structural_assign(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if doc.language != Language::Python || node.kind() != "assignment" {
        return;
    }
    let Some(ty_node) = node.child_by_field_name("type") else {
        return;
    };
    let Some(rhs) = node
        .child_by_field_name("right")
        .filter(|r| r.kind() == "call")
    else {
        return;
    };
    let Some(callee) = rhs.child_by_field_name("function") else {
        return;
    };
    let contract = normalize_contract_ident(node_text(ty_node, source));
    let impl_cls = normalize_contract_ident(node_text(callee, source));
    if !contract.is_empty()
        && impl_cls
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase())
    {
        doc.contract_impls.insert(ContractImpl {
            contract: String::from(contract),
            implementor: String::from(impl_cls),
        });
    }
}

fn record_python_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    record_py_structural_assign(node, source, doc);
    if node.kind() != "class_definition" || has_dataclass_decorator(node, source) {
        return;
    }
    let cls_name = field_name_string(node, "name", "<class>", source);
    let bases = collect_py_superclasses(node, source);
    let methods = count_named_kinds(
        node.child_by_field_name("body"),
        &["function_definition", "decorated_definition"],
    );
    if bases
        .iter()
        .any(|b| PY_ABSTRACT_BASES.contains(&b.as_str()))
        && methods >= 1
    {
        doc.contracts.push(ContractDecl {
            name: cls_name,
            method_count: methods,
            is_public: true,
            span: node_span(node),
        });
        return;
    }
    for base in bases {
        if !PY_DATA_OR_IGNORED_BASES.contains(&base.as_str()) {
            doc.contract_impls.insert(ContractImpl {
                contract: base,
                implementor: cls_name.clone(),
            });
        }
    }
}

/// Extracts behavioral contracts and concrete implementations (`GF075`) across Rust/TS/Python.
pub fn extract_contracts_and_impls(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_contracts(root, source, doc);
}

fn walk_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    record_rust_contracts(node, source, doc);
    record_ts_contracts(node, source, doc);
    record_python_contracts(node, source, doc);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_contracts(child, source, doc);
    }
}
