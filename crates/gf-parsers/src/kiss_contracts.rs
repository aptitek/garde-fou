//! Behavioral contract (`trait` / `interface` / `ABC` / `Protocol`) and concrete
//! implementation extractor (`GF075` YAGNI single-implementation check).

use gf_core::{ContractDecl, ContractImpl, UastDocument};
use tree_sitter::Node;

use crate::cst::{
    count_children_of_kind, count_named_kinds, field_name_string, node_span, node_text,
};

const PY_ABSTRACT_BASES: &[&str] = &["ABC", "Protocol"];
const PY_IGNORED_BASES: &[&str] = &[
    "ABC",
    "Protocol",
    "object",
    "Exception",
    "BaseException",
    "Enum",
    "IntEnum",
    "StrEnum",
];

fn normalize_contract_ident(raw: &str) -> &str {
    let before_generics = raw.split('<').next().unwrap_or(raw).trim();
    before_generics
        .rsplit("::")
        .next()
        .and_then(|s| s.rsplit('.').next())
        .unwrap_or(before_generics)
        .trim()
}

fn has_pub_visibility(node: Node<'_>, source: &str) -> bool {
    let mut c = node.walk();
    node.children(&mut c).any(|ch| {
        (ch.kind() == "visibility_modifier" && node_text(ch, source) == "pub")
            || node
                .parent()
                .is_some_and(|p| p.kind() == "export_statement")
    })
}

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

fn count_ts_interface_methods(body: Option<Node<'_>>) -> u32 {
    let direct = count_named_kinds(body, &["method_signature", "call_signature"]);
    let Some(b) = body else {
        return direct;
    };
    let mut fn_props = 0_u32;
    let mut cursor = b.walk();
    for ch in b.named_children(&mut cursor) {
        if ch.kind() == "property_signature"
            && ch
                .child_by_field_name("type")
                .is_some_and(|t| node_contains_kind(t, "function_type"))
        {
            fn_props = fn_props.saturating_add(1);
        }
    }
    direct.saturating_add(fn_props)
}

fn node_contains_kind(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut c = node.walk();
    node.children(&mut c).any(|ch| node_contains_kind(ch, kind))
}

fn collect_heritage_types(node: Node<'_>, source: &str, out: &mut Vec<String>) {
    if matches!(node.kind(), "type_identifier" | "identifier") {
        let name = normalize_contract_ident(node_text(node, source));
        if !name.is_empty() {
            out.push(String::from(name));
        }
        return;
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        if ch.kind() != "type_arguments" {
            collect_heritage_types(ch, source, out);
        }
    }
}

fn record_ts_class_impls(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let cls = field_name_string(node, "name", "<class>", source);
    let mut bases = Vec::new();
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        if matches!(
            ch.kind(),
            "class_heritage" | "implements_clause" | "extends_clause"
        ) {
            collect_heritage_types(ch, source, &mut bases);
        }
    }
    for contract in bases {
        doc.contract_impls.insert(ContractImpl {
            contract,
            implementor: cls.clone(),
        });
    }
}

fn record_ts_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "interface_declaration" => {
            let methods = count_ts_interface_methods(node.child_by_field_name("body"));
            if methods >= 1 {
                doc.contracts.push(ContractDecl {
                    name: field_name_string(node, "name", "<interface>", source),
                    method_count: methods,
                    is_public: has_pub_visibility(node, source),
                    span: node_span(node),
                });
            }
        }
        "abstract_class_declaration" => {
            let methods = count_named_kinds(
                node.child_by_field_name("body"),
                &["abstract_method_signature", "method_definition"],
            );
            if methods >= 1 {
                doc.contracts.push(ContractDecl {
                    name: field_name_string(node, "name", "<abstract>", source),
                    method_count: methods,
                    is_public: has_pub_visibility(node, source),
                    span: node_span(node),
                });
            }
        }
        "class_declaration" => record_ts_class_impls(node, source, doc),
        _ => {}
    }
}

fn collect_py_superclasses(node: Node<'_>, source: &str) -> Vec<String> {
    let Some(sup) = node.child_by_field_name("superclasses") else {
        return Vec::new();
    };
    let mut c = sup.walk();
    sup.named_children(&mut c)
        .map(|arg| normalize_contract_ident(node_text(arg, source)))
        .filter(|base| !base.is_empty())
        .map(String::from)
        .collect()
}

fn record_python_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if node.kind() != "class_definition" {
        return;
    }
    let cls_name = field_name_string(node, "name", "<class>", source);
    let bases = collect_py_superclasses(node, source);
    let methods = count_named_kinds(
        node.child_by_field_name("body"),
        &["function_definition", "decorated_definition"],
    );
    let is_abstract = bases
        .iter()
        .any(|b| PY_ABSTRACT_BASES.contains(&b.as_str()));
    if is_abstract && methods >= 1 {
        doc.contracts.push(ContractDecl {
            name: cls_name,
            method_count: methods,
            is_public: true,
            span: node_span(node),
        });
        return;
    }
    for base in bases {
        if !PY_IGNORED_BASES.contains(&base.as_str()) {
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
