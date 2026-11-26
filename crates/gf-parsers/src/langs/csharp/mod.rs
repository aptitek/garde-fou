//! Safe `Tree-sitter` extractor for C# (`.cs`).
//!
//! Extracts `namespace`-based `using` resolution, C# 12 primary constructor `CS9124`
//! double-capture detection, `record`/`partial` support, and `CS-*` / EF Core guardrails.

mod helpers;
pub mod quirks;

use gf_core::{
    CSharpViolation, CallableDecl, CallableKind, CompiledModuleSummary, ContractDecl, ContractImpl,
    ExportDecl, ImportDecl, KissYagniFinding, KissYagniKind, LangRuleFinding, Language, RepoPath,
    TypeDecl, TypeKind, UastDocument,
};
use tree_sitter::Node;

use self::helpers::{
    CS_COMPLEXITY, check_cs_ct_propagation, check_primary_ctor_capture, extract_cs_params,
    has_cs_modifier, has_cs_test_attr, inspect_cs_bypasses, record_cs_test_case,
};
use self::quirks::{CS_FLAG_ASYNC, CS_FLAG_ENTRY, CS_FLAG_TEST, CsWalkFlags, walk_cs_body};
use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{
    CallableSpec, assemble_callable, field_name_string, node_span, node_text,
    parse_and_init_document,
};
use crate::types::{LanguageExtractor, ParseError};

/// `Tree-sitter` AST extractor for C# source files (`.cs`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CSharpExtractor;

impl LanguageExtractor for CSharpExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_c_sharp::LANGUAGE.into();
        let (tree, mut doc) = parse_and_init_document(&ts_lang, path, Language::CSharp, source)?;
        let root = tree.root_node();
        let is_gen = path.as_str().ends_with(".g.cs") || path.as_str().ends_with(".Designer.cs");
        let mut meta = CompiledModuleSummary::default();
        if !is_gen {
            CsWalker {
                source,
                is_test: path.as_str().contains("Tests.cs") || path.as_str().contains("/Tests/"),
                is_entry: path.file_name() == "Program.cs",
                meta: &mut meta,
                doc: &mut doc,
            }
            .walk_top(root);
            inspect_cs_bypasses(root, source, &mut doc);
            populate_cst_fortress(root, source, &mut doc);
        }
        doc.compiled_meta = Some(meta);
        Ok(doc)
    }
}

struct CsWalker<'a> {
    source: &'a str,
    is_test: bool,
    is_entry: bool,
    meta: &'a mut CompiledModuleSummary,
    doc: &'a mut UastDocument,
}

impl CsWalker<'_> {
    fn walk_top(&mut self, node: Node<'_>) {
        let mut cursor = node.walk();
        for ch in node.named_children(&mut cursor) {
            match ch.kind() {
                "file_scoped_namespace_declaration" | "namespace_declaration" => {
                    if let Some(nm) = ch.child_by_field_name("name") {
                        self.meta.package_or_namespace =
                            Some(String::from(node_text(nm, self.source).trim()));
                    }
                    self.walk_top(ch);
                }
                "declaration_list" => self.walk_top(ch),
                "using_directive" => self.push_using(ch),
                "class_declaration"
                | "struct_declaration"
                | "record_declaration"
                | "interface_declaration" => self.extract_type(ch),
                "local_function_statement" => self.extract_method(ch),
                _ => {}
            }
        }
    }

    fn push_using(&mut self, ch: Node<'_>) {
        let raw = node_text(ch, self.source)
            .trim()
            .trim_start_matches("global ")
            .trim_start_matches("using ")
            .trim_start_matches("static ")
            .trim_end_matches(';')
            .trim();
        let target = raw.split_once('=').map_or(raw, |(_, rhs)| rhs.trim());
        if !target.is_empty() {
            self.doc.imports.push(ImportDecl {
                module_source: String::from(target),
                symbols: Vec::new(),
                span: node_span(ch),
                is_type_only: false,
            });
        }
    }

    fn extract_method(&mut self, node: Node<'_>) {
        let name = field_name_string(node, "name", "<anon>", self.source);
        let is_async = has_cs_modifier(node, self.source, "async");
        let ret_txt = node
            .child_by_field_name("returns")
            .or_else(|| node.child_by_field_name("type"))
            .map_or("", |r| node_text(r, self.source).trim());
        let is_test = self.is_test || has_cs_test_attr(node, self.source);
        if is_async && ret_txt == "void" && !is_test {
            self.doc.lang_rule_findings.push(LangRuleFinding::new(
                CSharpViolation::NoAsyncVoid,
                node_span(node),
                format!("`async void {name}` swallows exceptions; return `Task` or `ValueTask`"),
            ));
        }
        let (param_count, bool_params, ct_name) = extract_cs_params(node, self.source);
        let body = node.child_by_field_name("body");
        let spec = CallableSpec {
            name: name.clone(),
            kind: CallableKind::Method,
            decl_node: node,
            body_node: body,
            param_count,
        };
        let callable: CallableDecl = assemble_callable(spec, self.source, &CS_COMPLEXITY);
        if bool_params > 1 {
            self.doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::ExcessiveBoolParams,
                span: node_span(node),
                measured: bool_params,
                detail: format!("C# callable `{name}` has {bool_params} bool parameters"),
            });
        }
        self.doc.callables.push(callable);
        let Some(b) = body else { return };
        let bits = (u8::from(is_async) * CS_FLAG_ASYNC)
            | (u8::from(is_test) * CS_FLAG_TEST)
            | (u8::from(self.is_entry || name == "Main") * CS_FLAG_ENTRY);
        walk_cs_body(b, self.source, CsWalkFlags::from_bits(bits), self.doc);
        let btxt = node_text(b, self.source);
        if is_async && let Some(ct) = ct_name.as_deref() {
            check_cs_ct_propagation(node_span(node), btxt, ct, self.doc);
        }
        if is_test {
            record_cs_test_case(node_span(node), btxt, name, self.doc);
        }
    }

    fn extract_type(&mut self, node: Node<'_>) {
        let name = field_name_string(node, "name", "<anon>", self.source);
        let is_iface = node.kind() == "interface_declaration";
        let is_record = node.kind() == "record_declaration";
        let is_abstract = has_cs_modifier(node, self.source, "abstract");
        let is_public = has_cs_modifier(node, self.source, "public");
        if is_public && !self.is_test && !self.is_entry && !name.ends_with("Controller") {
            self.doc.exports.push(ExportDecl {
                symbol: name.clone(),
                span: node_span(node),
                is_crate_internal: false,
                is_type_only: false,
            });
        }
        check_primary_ctor_capture(node, self.source, self.doc);
        let (method_count, abstract_methods) = self.inspect_type_body(node, &name);
        if !is_record && (is_iface || is_abstract) && abstract_methods >= 1 {
            self.doc.contracts.push(ContractDecl {
                name: name.clone(),
                span: node_span(node),
                method_count: abstract_methods,
                is_public,
            });
        }
        self.doc.types.push(TypeDecl {
            name,
            kind: if is_iface {
                TypeKind::Interface
            } else {
                TypeKind::Class
            },
            span: node_span(node),
            field_count: 0,
            bool_field_count: 0,
            method_count,
        });
    }

    fn record_base_list(&mut self, base_list: Node<'_>, name: &str) {
        let mut bc = base_list.walk();
        for b in base_list.named_children(&mut bc) {
            let base = node_text(b, self.source)
                .split('<')
                .next()
                .unwrap_or("")
                .trim();
            if !base.is_empty() {
                self.doc.contract_impls.insert(ContractImpl {
                    contract: String::from(base),
                    implementor: String::from(name),
                });
            }
        }
    }

    fn inspect_decl_list(&mut self, decl_list: Node<'_>) -> (u32, u32) {
        let (mut method_count, mut abstract_methods) = (0_u32, 0_u32);
        let mut dc = decl_list.walk();
        for member in decl_list.named_children(&mut dc) {
            if member.kind() != "method_declaration" {
                continue;
            }
            method_count += 1;
            let is_abs = member.child_by_field_name("body").is_none()
                || has_cs_modifier(member, self.source, "abstract");
            abstract_methods += u32::from(is_abs);
            self.extract_method(member);
        }
        (method_count, abstract_methods)
    }

    fn inspect_type_body(&mut self, node: Node<'_>, name: &str) -> (u32, u32) {
        let (mut method_count, mut abstract_methods) = (0_u32, 0_u32);
        let mut c = node.walk();
        for ch in node.named_children(&mut c) {
            match ch.kind() {
                "base_list" => self.record_base_list(ch, name),
                "declaration_list" => {
                    let (m, a) = self.inspect_decl_list(ch);
                    method_count += m;
                    abstract_methods += a;
                }
                _ => {}
            }
        }
        (method_count, abstract_methods)
    }
}
