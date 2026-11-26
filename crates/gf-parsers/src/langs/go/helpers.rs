//! Pure CST helpers for Go types, interfaces, parameters, imports, and `var _ I = (*T)(nil)`.

use std::collections::BTreeSet;

use gf_core::{
    ContractImpl, GoViolation, ImportDecl, LangRuleFinding, TypeDecl, TypeKind, UastDocument,
};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

fn collect_type_decl_ifaces(decl: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    let mut tc = decl.walk();
    for spec in decl.named_children(&mut tc) {
        if spec.kind() == "type_spec"
            && spec
                .child_by_field_name("type")
                .is_some_and(|t| t.kind() == "interface_type")
            && let Some(name) = spec.child_by_field_name("name")
        {
            out.insert(String::from(node_text(name, source).trim()));
        }
    }
}

pub fn collect_file_interfaces(root: Node<'_>, source: &str, out: &mut BTreeSet<String>) {
    let mut cursor = root.walk();
    for ch in root.named_children(&mut cursor) {
        if ch.kind() == "type_declaration" {
            collect_type_decl_ifaces(ch, source, out);
        }
    }
}

pub fn is_go_exported_ident(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
        && !matches!(name, "main" | "init")
        && !["Test", "Benchmark", "Fuzz", "Example"]
            .iter()
            .any(|p| name.starts_with(p))
}

pub fn count_go_params(params_node: Option<Node<'_>>, source: &str) -> (u32, u32) {
    let Some(params) = params_node else {
        return (0, 0);
    };
    let (mut total, mut bools) = (0_u32, 0_u32);
    let mut cursor = params.walk();
    for ch in params.named_children(&mut cursor) {
        if !matches!(
            ch.kind(),
            "parameter_declaration" | "variadic_parameter_declaration"
        ) {
            continue;
        }
        let mut pc = ch.walk();
        let names = ch
            .named_children(&mut pc)
            .filter(|sub| sub.kind() == "identifier")
            .count();
        let count = u32::try_from(names.max(1)).unwrap_or(u32::MAX);
        total += count;
        if ch
            .child_by_field_name("type")
            .is_some_and(|t| node_text(t, source).trim() == "bool")
        {
            bools += count;
        }
    }
    (total, bools)
}

pub fn extract_receiver_type(node: Node<'_>, source: &str) -> Option<String> {
    let recv = node.child_by_field_name("receiver")?;
    let mut cursor = recv.walk();
    for ch in recv.named_children(&mut cursor) {
        if ch.kind() == "parameter_declaration"
            && let Some(ty) = ch.child_by_field_name("type")
        {
            let raw = node_text(ty, source).trim().trim_start_matches('*');
            let base = raw.split('[').next().unwrap_or(raw).trim();
            if !base.is_empty() {
                return Some(String::from(base));
            }
        }
    }
    None
}

fn inspect_struct_field(
    field: Node<'_>,
    struct_name: &str,
    source: &str,
    doc: &mut UastDocument,
) -> bool {
    let Some(ft) = field.child_by_field_name("type") else {
        return false;
    };
    let ftxt = node_text(ft, source).trim();
    if ftxt == "context.Context" {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            GoViolation::ContextMisuse,
            node_span(field),
            format!("do not store `context.Context` in struct `{struct_name}`"),
        ));
    }
    ftxt == "bool"
}

pub fn extract_go_struct(spec: Node<'_>, name: &str, source: &str, doc: &mut UastDocument) {
    let Some(ty) = spec.child_by_field_name("type") else {
        return;
    };
    let (mut fields, mut bools) = (0_u32, 0_u32);
    let mut stack = vec![ty];
    while let Some(n) = stack.pop() {
        if n.kind() == "field_declaration" {
            fields += 1;
            bools += u32::from(inspect_struct_field(n, name, source, doc));
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
    doc.types.push(TypeDecl {
        name: String::from(name),
        kind: TypeKind::Struct,
        span: node_span(spec),
        field_count: fields,
        bool_field_count: bools,
        method_count: 0,
    });
}

fn extract_go_iface_methods(ty: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut methods = BTreeSet::new();
    let mut c = ty.walk();
    for ch in ty.named_children(&mut c) {
        if ch.kind() == "method_elem"
            && let Some(mn) = ch.child_by_field_name("name")
        {
            methods.insert(String::from(node_text(mn, source).trim()));
        }
    }
    methods
}

pub fn extract_go_imports(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind() == "import_spec"
            && let Some(p) = n.child_by_field_name("path")
        {
            let raw = node_text(p, source).trim().trim_matches('"');
            doc.imports.push(ImportDecl {
                module_source: String::from(raw),
                symbols: Vec::new(),
                span: node_span(n),
                is_type_only: false,
            });
        }
        let mut c = n.walk();
        for ch in n.named_children(&mut c) {
            stack.push(ch);
        }
    }
}

pub fn extract_go_interface_witness(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if !node_text(node, source).contains("var _") {
        return;
    }
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind() == "var_spec"
            && n.child_by_field_name("name")
                .is_some_and(|nm| node_text(nm, source).trim() == "_")
            && let Some(ty) = n.child_by_field_name("type")
            && let Some(val) = n.child_by_field_name("value")
        {
            let contract = node_text(ty, source).trim();
            let vtxt = node_text(val, source);
            if let Some(inner) = vtxt.split("(*").nth(1).and_then(|s| s.split(')').next()) {
                doc.contract_impls.insert(ContractImpl {
                    contract: String::from(contract),
                    implementor: String::from(inner.trim()),
                });
            }
        }
        let mut c = n.walk();
        for ch in n.named_children(&mut c) {
            stack.push(ch);
        }
    }
}

pub fn record_go_test_case(span: gf_core::Span, btxt: &str, name: String, doc: &mut UastDocument) {
    const MARKERS: &[&str] = &["t.Fatal", "t.Error", "t.Fail", "require.", "assert."];
    doc.test_cases
        .push(crate::cst::make_test_case(name, span, btxt, MARKERS));
}

pub fn record_go_interface(
    spec: Node<'_>,
    source: &str,
    meta: &mut gf_core::CompiledModuleSummary,
    doc: &mut UastDocument,
) {
    let Some(ty) = spec.child_by_field_name("type") else {
        return;
    };
    let methods = extract_go_iface_methods(ty, source);
    if methods.is_empty() {
        return;
    }
    let name = crate::cst::field_name_string(spec, "name", "<anon>", source);
    let span = node_span(spec);
    let method_count = u32::try_from(methods.len()).unwrap_or(u32::MAX);
    doc.contracts.push(gf_core::ContractDecl {
        name: name.clone(),
        span,
        method_count,
        is_public: is_go_exported_ident(&name),
    });
    meta.go_interfaces.push(gf_core::StructuralInterfaceDecl {
        name,
        span,
        methods,
    });
}
