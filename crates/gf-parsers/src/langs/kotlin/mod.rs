//! Safe `Tree-sitter` extractor for Kotlin (`.kt`, `.kts`).
//!
//! Extracts `package`-based imports (`GF071`/`GF072`), ADT `sealed`/`data`/`value` exemptions
//! (`GF075`), and `KT-*` coroutine/null-safety quirks (`GF080`).

mod helpers;
pub mod quirks;

use gf_core::{
    CallableDecl, CallableKind, CompiledModuleSummary, ContractDecl, ContractImpl, ExportDecl,
    KissYagniFinding, KissYagniKind, Language, RepoPath, TypeDecl, TypeKind, UastDocument,
};
use tree_sitter::Node;

use self::helpers::{
    KT_COMPLEXITY, check_kt_fn_header, count_kt_params, find_child_of_kind, find_child_text,
    has_annotation, has_modifier, is_kt_public, push_kt_import, record_kt_test_case,
};
use self::quirks::{
    KT_FLAG_IS_MAIN_OR_KTS, KT_FLAG_IS_TEST, KT_FLAG_SUSPEND, KtWalkFlags, inspect_kt_bypasses,
    walk_kt_body,
};
use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{CallableSpec, assemble_callable, node_span, node_text, parse_and_init_document};
use crate::types::{LanguageExtractor, ParseError};

/// `Tree-sitter` AST extractor for Kotlin source and script files (`.kt`, `.kts`).
#[derive(Clone, Copy, Debug, Default)]
pub struct KotlinExtractor;

impl LanguageExtractor for KotlinExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_kotlin_ng::LANGUAGE.into();
        let (tree, mut doc) = parse_and_init_document(&ts_lang, path, Language::Kotlin, source)?;
        KtWalker::run(tree.root_node(), path, source, &mut doc);
        Ok(doc)
    }
}

struct KtWalker<'a> {
    source: &'a str,
    is_test: bool,
    is_kts: bool,
    meta: &'a mut CompiledModuleSummary,
    doc: &'a mut UastDocument,
}

impl KtWalker<'_> {
    fn run(root: Node<'_>, path: &RepoPath, source: &str, doc: &mut UastDocument) {
        let mut meta = CompiledModuleSummary::default();
        let is_test = path.as_str().contains("Test.kt") || path.as_str().contains("/test/");
        let is_kts = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("kts"));
        KtWalker {
            source,
            is_test,
            is_kts,
            meta: &mut meta,
            doc,
        }
        .walk_top(root);
        inspect_kt_bypasses(root, source, doc);
        populate_cst_fortress(root, source, doc);
        doc.compiled_meta = Some(meta);
    }
    fn walk_top(&mut self, root: Node<'_>) {
        let mut cursor = root.walk();
        for ch in root.named_children(&mut cursor) {
            match ch.kind() {
                "package_header" => {
                    let pkg = node_text(ch, self.source)
                        .trim()
                        .trim_start_matches("package")
                        .trim()
                        .trim_end_matches(';')
                        .trim();
                    self.meta.package_or_namespace = Some(String::from(pkg));
                }
                "import_list" => {
                    let mut ic = ch.walk();
                    for imp in ch.named_children(&mut ic) {
                        push_kt_import(imp, self.source, self.doc);
                    }
                }
                "import_header" | "import" => push_kt_import(ch, self.source, self.doc),
                "function_declaration" => self.extract_function(ch, true),
                "class_declaration" => self.extract_class(ch),
                _ => {}
            }
        }
    }

    fn maybe_export_fn(&mut self, node: Node<'_>, name: &str, is_test_fn: bool) {
        let excluded = self.is_kts
            || is_test_fn
            || name == "main"
            || has_annotation(node, self.source, "@Preview");
        if !excluded && is_kt_public(node, self.source) {
            self.doc.exports.push(ExportDecl {
                symbol: String::from(name),
                span: node_span(node),
                is_crate_internal: false,
                is_type_only: false,
            });
        }
    }

    fn extract_function(&mut self, node: Node<'_>, is_top_level: bool) {
        let name = find_child_text(node, "identifier", self.source).unwrap_or("<anon>");
        let is_suspend = has_modifier(node, self.source, "suspend");
        let is_test_fn = self.is_test || has_annotation(node, self.source, "@Test");
        let (param_count, bool_params) = count_kt_params(node, self.source, self.doc);
        let body = find_child_of_kind(node, "function_body");
        let header_end = body.map_or_else(|| node.end_byte(), |b| b.start_byte());
        let header_txt = self.source.get(node.start_byte()..header_end).unwrap_or("");
        check_kt_fn_header(node_span(node), header_txt, is_suspend, self.doc);
        if is_top_level {
            self.maybe_export_fn(node, name, is_test_fn);
        }
        let kind = if is_top_level {
            CallableKind::Function
        } else {
            CallableKind::Method
        };
        let spec = CallableSpec {
            name: String::from(name),
            kind,
            decl_node: node,
            body_node: body,
            param_count,
        };
        let callable: CallableDecl = assemble_callable(spec, self.source, &KT_COMPLEXITY);
        if bool_params > 1 {
            self.doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::ExcessiveBoolParams,
                span: node_span(node),
                measured: bool_params,
                detail: format!("Kotlin callable `{name}` has {bool_params} Boolean parameters"),
            });
        }
        self.doc.callables.push(callable);
        if let Some(b) = body {
            let bits = (u8::from(is_suspend) * KT_FLAG_SUSPEND)
                | (u8::from(is_test_fn) * KT_FLAG_IS_TEST)
                | (u8::from(name == "main" || self.is_kts) * KT_FLAG_IS_MAIN_OR_KTS);
            walk_kt_body(b, self.source, KtWalkFlags::from_bits(bits), self.doc);
            if is_test_fn {
                record_kt_test_case(node_span(node), node_text(b, self.source), name, self.doc);
            }
        }
    }

    fn extract_class(&mut self, node: Node<'_>) {
        let name = find_child_text(node, "identifier", self.source).unwrap_or("<anon>");
        let is_adt = ["sealed", "data", "value", "enum", "annotation"]
            .iter()
            .any(|kw| has_modifier(node, self.source, kw));
        let is_iface = node_text(node, self.source)
            .split('{')
            .next()
            .is_some_and(|h| h.contains("interface "));
        let is_abstract = has_modifier(node, self.source, "abstract");
        let is_pub = is_kt_public(node, self.source);
        if !self.is_kts && !self.is_test && is_pub {
            self.doc.exports.push(ExportDecl {
                symbol: String::from(name),
                span: node_span(node),
                is_crate_internal: false,
                is_type_only: false,
            });
        }
        let (method_count, abstract_methods) = self.inspect_class_members(node, name);
        if !is_adt && (is_iface || is_abstract) && abstract_methods >= 1 {
            self.doc.contracts.push(ContractDecl {
                name: String::from(name),
                span: node_span(node),
                method_count: abstract_methods,
                is_public: is_pub,
            });
        }
        let kind = if is_iface {
            TypeKind::Interface
        } else {
            TypeKind::Class
        };
        self.doc.types.push(TypeDecl {
            name: String::from(name),
            kind,
            span: node_span(node),
            field_count: 0,
            bool_field_count: 0,
            method_count,
        });
    }

    fn inspect_class_body(&mut self, body: Node<'_>) -> (u32, u32) {
        let (mut methods, mut abstract_methods) = (0_u32, 0_u32);
        let mut bc = body.walk();
        for member in body.named_children(&mut bc) {
            if member.kind() != "function_declaration" {
                continue;
            }
            methods += 1;
            let is_abs = find_child_of_kind(member, "function_body").is_none()
                || has_modifier(member, self.source, "abstract");
            abstract_methods += u32::from(is_abs);
            self.extract_function(member, false);
        }
        (methods, abstract_methods)
    }

    fn inspect_class_members(&mut self, node: Node<'_>, class_name: &str) -> (u32, u32) {
        let (mut methods, mut abstract_methods) = (0_u32, 0_u32);
        let mut c = node.walk();
        for ch in node.named_children(&mut c) {
            match ch.kind() {
                "delegation_specifier" => {
                    let base = node_text(ch, self.source)
                        .split(['(', '<'])
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !base.is_empty() {
                        self.doc.contract_impls.insert(ContractImpl {
                            contract: String::from(base),
                            implementor: String::from(class_name),
                        });
                    }
                }
                "class_body" => {
                    let (m, a) = self.inspect_class_body(ch);
                    methods += m;
                    abstract_methods += a;
                }
                _ => {}
            }
        }
        (methods, abstract_methods)
    }
}
