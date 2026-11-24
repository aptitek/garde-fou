//! TypeScript contract (`interface`, behavioral `type`, `abstract class`) and
//! structural implementation (`implements`, typed object literal, `satisfies`) extractor.

use gf_core::{ContractDecl, ContractImpl, UastDocument};
use tree_sitter::Node;

use crate::cst::{count_named_kinds, field_name_string, node_span, node_text};

pub(super) fn normalize_contract_ident(raw: &str) -> &str {
    let clean = raw.trim().trim_start_matches(':').trim();
    let before_generics = clean.split('<').next().unwrap_or(clean).trim();
    before_generics
        .rsplit("::")
        .next()
        .and_then(|s| s.rsplit('.').next())
        .unwrap_or(before_generics)
        .trim()
}

pub(super) fn has_pub_visibility(node: Node<'_>, source: &str) -> bool {
    if node
        .parent()
        .is_some_and(|p| p.kind() == "export_statement")
    {
        return true;
    }
    let mut c = node.walk();
    node.children(&mut c)
        .any(|ch| ch.kind() == "visibility_modifier" && node_text(ch, source) == "pub")
}

fn node_contains_kind(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut c = node.walk();
    node.children(&mut c).any(|ch| node_contains_kind(ch, kind))
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

fn record_ts_value_impls(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if node.kind() == "variable_declarator"
        && let Some(ty_node) = node.child_by_field_name("type")
        && node
            .child_by_field_name("value")
            .is_some_and(|v| v.kind() == "object")
    {
        let contract = normalize_contract_ident(node_text(ty_node, source));
        let implementor = field_name_string(node, "name", "<object>", source);
        if !contract.is_empty() {
            doc.contract_impls.insert(ContractImpl {
                contract: String::from(contract),
                implementor,
            });
        }
    } else if node.kind() == "satisfies_expression" {
        let mut c = node.walk();
        if let Some(ty_node) = node.named_children(&mut c).last() {
            let contract = normalize_contract_ident(node_text(ty_node, source));
            let implementor = node
                .parent()
                .filter(|p| p.kind() == "variable_declarator")
                .map_or_else(
                    || String::from("<satisfies>"),
                    |p| field_name_string(p, "name", "<satisfies>", source),
                );
            if !contract.is_empty() {
                doc.contract_impls.insert(ContractImpl {
                    contract: String::from(contract),
                    implementor,
                });
            }
        }
    }
}

fn push_ts_contract(
    node: Node<'_>,
    fallback: &str,
    methods: u32,
    source: &str,
) -> Option<ContractDecl> {
    (methods >= 1).then(|| ContractDecl {
        name: field_name_string(node, "name", fallback, source),
        method_count: methods,
        is_public: has_pub_visibility(node, source),
        span: node_span(node),
    })
}

pub(super) fn record_ts_contracts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let maybe_decl = match node.kind() {
        "interface_declaration" => push_ts_contract(
            node,
            "<interface>",
            count_ts_interface_methods(node.child_by_field_name("body")),
            source,
        ),
        "type_alias_declaration" => node
            .child_by_field_name("value")
            .filter(|v| v.kind() == "object_type")
            .and_then(|val| {
                push_ts_contract(
                    node,
                    "<type>",
                    count_ts_interface_methods(Some(val)),
                    source,
                )
            }),
        "abstract_class_declaration" => push_ts_contract(
            node,
            "<abstract>",
            count_named_kinds(
                node.child_by_field_name("body"),
                &["abstract_method_signature", "method_definition"],
            ),
            source,
        ),
        "class_declaration" => {
            record_ts_class_impls(node, source, doc);
            None
        }
        "variable_declarator" | "satisfies_expression" => {
            record_ts_value_impls(node, source, doc);
            None
        }
        _ => None,
    };
    if let Some(decl) = maybe_decl {
        doc.contracts.push(decl);
    }
}
