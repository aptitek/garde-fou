//! Safe `Tree-sitter` extractor for Go (`.go`).
//!
//! Extracts package-level exports, implicit interface metadata (`GF075`), `go.mod`-compatible
//! imports (`GF071`/`GF072`), and `GO-*` quirks (`GF080`).

mod helpers;
pub mod quirks;
mod quirks_calls;

use std::collections::BTreeSet;

use gf_core::{
    CallableDecl, CallableKind, CompiledModuleSummary, ConcreteTypeMethods, ExportDecl,
    KissYagniFinding, KissYagniKind, Language, RepoPath, UastDocument,
};
use tree_sitter::Node;

use self::helpers::{
    collect_file_interfaces, count_go_params, extract_go_imports, extract_go_interface_witness,
    extract_go_struct, extract_receiver_type, is_go_exported_ident, record_go_interface,
    record_go_test_case,
};
use self::quirks::{
    GO_FLAG_HAS_CTX, GO_FLAG_IFACE_RET, GO_FLAG_IS_CMD, GO_FLAG_IS_TEST, GoFnCtx,
    check_go_context_params, inspect_go_body, inspect_go_comments,
};
use crate::anti_cheat::populate_cst_fortress;
use crate::complexity::ComplexityRules;
use crate::cst::{CallableSpec, assemble_callable, field_name_string, node_span, node_text};
use crate::types::{LanguageExtractor, ParseError};

const GO_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &["if_statement", "for_statement"],
    flat_branches: &[],
    switch_nodes: &[
        "expression_switch_statement",
        "type_switch_statement",
        "select_statement",
    ],
    case_arm_nodes: &[
        "expression_case",
        "type_case",
        "communication_case",
        "default_case",
    ],
    subtract_first_arm: true,
    bool_expr_nodes: &["binary_expression"],
    bool_ops: &["&&", "||"],
    nested_fn_nodes: &["func_literal"],
    else_wrapper_nodes: &[],
};

/// `Tree-sitter` AST extractor for Go source files (`.go`).
#[derive(Clone, Copy, Debug, Default)]
pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_go::LANGUAGE.into();
        let (tree, mut doc) =
            crate::cst::parse_and_init_document(&ts_lang, path, Language::Go, source)?;
        let root = tree.root_node();
        let is_generated = source
            .lines()
            .take(5)
            .any(|l| l.contains("Code generated") && l.contains("DO NOT EDIT"));
        let mut meta = CompiledModuleSummary::default();
        if !is_generated {
            let mut iface_names = BTreeSet::new();
            collect_file_interfaces(root, source, &mut iface_names);
            GoWalkCtx {
                source,
                path,
                iface_names: &iface_names,
                meta: &mut meta,
                doc: &mut doc,
            }
            .walk_top(root);
            inspect_go_comments(root, source, &mut doc);
            populate_cst_fortress(root, source, &mut doc);
        }
        doc.compiled_meta = Some(meta);
        Ok(doc)
    }
}

struct GoWalkCtx<'a> {
    source: &'a str,
    path: &'a RepoPath,
    iface_names: &'a BTreeSet<String>,
    meta: &'a mut CompiledModuleSummary,
    doc: &'a mut UastDocument,
}

impl GoWalkCtx<'_> {
    fn is_test(&self) -> bool {
        self.path.as_str().ends_with("_test.go")
    }

    fn is_cmd(&self) -> bool {
        self.path.as_str().starts_with("cmd/") || self.path.file_name() == "main.go"
    }

    fn walk_top(&mut self, root: Node<'_>) {
        let mut cursor = root.walk();
        for ch in root.named_children(&mut cursor) {
            match ch.kind() {
                "package_clause" => {
                    let pkg = node_text(ch, self.source)
                        .trim()
                        .trim_start_matches("package")
                        .trim();
                    self.meta.package_or_namespace = Some(String::from(pkg));
                }
                "import_declaration" => extract_go_imports(ch, self.source, self.doc),
                "function_declaration" | "method_declaration" => self.extract_callable(ch),
                "type_declaration" => self.extract_type_decl(ch),
                "var_declaration" => extract_go_interface_witness(ch, self.source, self.doc),
                _ => {}
            }
        }
    }

    fn record_receiver_or_export(&mut self, node: Node<'_>, name: &str) {
        if node.kind() == "method_declaration" {
            if let Some(recv) = extract_receiver_type(node, self.source) {
                if let Some(ct) = self
                    .meta
                    .go_concrete_types
                    .iter_mut()
                    .find(|c| c.type_name == recv)
                {
                    ct.methods.insert(String::from(name));
                } else {
                    let mut methods = BTreeSet::new();
                    methods.insert(String::from(name));
                    self.meta.go_concrete_types.push(ConcreteTypeMethods {
                        type_name: recv,
                        methods,
                    });
                }
            }
        } else if !self.is_test() && is_go_exported_ident(name) {
            self.doc.exports.push(ExportDecl {
                symbol: String::from(name),
                span: node_span(node),
                is_crate_internal: false,
                is_type_only: false,
            });
        }
    }

    fn extract_callable(&mut self, node: Node<'_>) {
        let is_method = node.kind() == "method_declaration";
        let name = field_name_string(node, "name", "<anon>", self.source);
        let params = node.child_by_field_name("parameters");
        let (param_count, bool_params) = count_go_params(params, self.source);
        self.record_receiver_or_export(node, &name);

        let body = node.child_by_field_name("body");
        let spec = CallableSpec {
            name: name.clone(),
            kind: if is_method {
                CallableKind::Method
            } else {
                CallableKind::Function
            },
            decl_node: node,
            body_node: body,
            param_count,
        };
        let callable: CallableDecl = assemble_callable(spec, self.source, &GO_COMPLEXITY);
        if bool_params > 1 {
            self.doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::ExcessiveBoolParams,
                span: node_span(node),
                measured: bool_params,
                detail: format!("Go callable `{name}` has {bool_params} boolean parameters"),
            });
        }
        self.doc.callables.push(callable);
        let has_ctx = check_go_context_params(params, self.source, self.doc);
        let ret_txt = node
            .child_by_field_name("result")
            .map_or("", |r| node_text(r, self.source));
        let returns_iface =
            ret_txt.contains("error") || self.iface_names.iter().any(|i| ret_txt.contains(i));
        if let Some(b) = body {
            let bits = (u8::from(returns_iface) * GO_FLAG_IFACE_RET)
                | (u8::from(has_ctx) * GO_FLAG_HAS_CTX)
                | (u8::from(self.is_test()) * GO_FLAG_IS_TEST)
                | (u8::from(self.is_cmd()) * GO_FLAG_IS_CMD);
            inspect_go_body(b, self.source, GoFnCtx::new(&name, bits), self.doc);
            if self.is_test() && name.starts_with("Test") {
                record_go_test_case(node_span(node), node_text(b, self.source), name, self.doc);
            }
        }
    }

    fn extract_type_decl(&mut self, decl: Node<'_>) {
        let mut tc = decl.walk();
        for spec in decl.named_children(&mut tc) {
            if spec.kind() != "type_spec" {
                continue;
            }
            let name = field_name_string(spec, "name", "<anon>", self.source);
            let exported = is_go_exported_ident(&name);
            if !self.is_test() && exported {
                self.doc.exports.push(ExportDecl {
                    symbol: name.clone(),
                    span: node_span(spec),
                    is_crate_internal: false,
                    is_type_only: false,
                });
            }
            let Some(ty) = spec.child_by_field_name("type") else {
                continue;
            };
            match ty.kind() {
                "struct_type" => extract_go_struct(spec, &name, self.source, self.doc),
                "interface_type" => {
                    record_go_interface(spec, self.source, self.meta, self.doc);
                }
                _ => {}
            }
        }
    }
}
