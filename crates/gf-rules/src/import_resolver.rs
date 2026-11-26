//! Pure `#![no_std]` cross-language workspace import resolver shared by Tarjan (`GF071`)
//! and architectural `[[boundaries]]` (`GF072`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{ImportDecl, Language, RepoPath, UastDocument};

use crate::compiled_import_resolver::CompiledWorkspaceIndex;
pub use crate::rust_import_resolver::normalize_join;
use crate::rust_import_resolver::{dir_of, push_known, resolve_rust_import};
use crate::ts_import_resolver::TsWorkspaceAliases;

/// Pre-indexed workspace import resolution table (`path -> doc_idx` + `tsconfig` + Go/Kotlin/C# indices).
pub struct WorkspaceImportResolver<'a> {
    path_index: BTreeMap<&'a str, usize>,
    ts_aliases: TsWorkspaceAliases,
    compiled: CompiledWorkspaceIndex,
}

impl<'a> WorkspaceImportResolver<'a> {
    /// Builds a deterministic import resolver over `docs`.
    #[must_use]
    pub fn new(docs: &'a [UastDocument]) -> Self {
        let path_index = docs
            .iter()
            .enumerate()
            .map(|(idx, d)| (d.path.as_str(), idx))
            .collect();
        let ts_aliases = TsWorkspaceAliases::from_docs(docs);
        let compiled = CompiledWorkspaceIndex::from_docs(docs);
        Self {
            path_index,
            ts_aliases,
            compiled,
        }
    }

    /// Resolves `imp` inside `(from_path, lang)` to matching target document indices in `docs`.
    #[must_use]
    pub fn resolve(&self, from_path: &RepoPath, lang: Language, imp: &ImportDecl) -> Vec<usize> {
        let mut out = Vec::new();
        let raw_from = from_path.as_str();
        match lang {
            Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx => {
                out = self
                    .ts_aliases
                    .resolve_into(raw_from, &imp.module_source, &self.path_index);
            }
            Language::Python => resolve_py_import(raw_from, imp, &self.path_index, &mut out),
            Language::Rust => {
                resolve_rust_import(raw_from, &imp.module_source, &self.path_index, &mut out);
            }
            Language::Go => self.compiled.resolve_go(&imp.module_source, &mut out),
            Language::Kotlin => self.compiled.resolve_kotlin(&imp.module_source, &mut out),
            Language::CSharp => self.compiled.resolve_csharp(&imp.module_source, &mut out),
            Language::Php => {
                resolve_php_psr4_import(&imp.module_source, &self.path_index, &mut out);
            }
            Language::Dart => {
                resolve_dart_import(raw_from, &imp.module_source, &self.path_index, &mut out);
            }
            Language::McFunction => {
                resolve_mcfunction_call(&imp.module_source, &self.path_index, &mut out);
            }
            _ => {}
        }
        out.retain(|&idx| self.path_index.get(raw_from) != Some(&idx));
        out
    }
}

fn py_base_prefixes(from_path: &str, dots: usize, tail: &str) -> Vec<String> {
    if dots >= 1 {
        let mut cur = dir_of(from_path);
        for _ in 1..dots {
            cur = dir_of(cur);
        }
        let joined = if tail.is_empty() {
            String::from(cur)
        } else {
            normalize_join(cur, tail)
        };
        alloc::vec![joined]
    } else if tail.is_empty() {
        Vec::new()
    } else {
        alloc::vec![String::from(tail), format!("src/{tail}")]
    }
}

fn probe_py_submodules(
    pfx: &str,
    symbols: &[String],
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) -> bool {
    let before = out.len();
    for sym in symbols {
        let sym_rel = sym.trim().replace('.', "/");
        if sym_rel.is_empty() || sym_rel == "*" {
            continue;
        }
        let sub = if pfx.is_empty() {
            sym_rel
        } else {
            format!("{pfx}/{sym_rel}")
        };
        push_known(&format!("{sub}.py"), path_index, out);
        push_known(&format!("{sub}/__init__.py"), path_index, out);
    }
    out.len() > before
}

fn resolve_py_import(
    from_path: &str,
    imp: &ImportDecl,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let clean = imp
        .module_source
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim();
    let dots = clean.bytes().take_while(|&b| b == b'.').count();
    let tail = clean.get(dots..).unwrap_or("").replace('.', "/");
    for pfx in py_base_prefixes(from_path, dots, &tail) {
        if !pfx.is_empty() {
            push_known(&format!("{pfx}.py"), path_index, out);
        }
        let matched_sub = probe_py_submodules(&pfx, &imp.symbols, path_index, out);
        if !matched_sub && !pfx.is_empty() {
            push_known(&format!("{pfx}/__init__.py"), path_index, out);
        }
    }
}

fn resolve_php_psr4_import(spec: &str, path_index: &BTreeMap<&str, usize>, out: &mut Vec<usize>) {
    let rel = spec.trim().trim_start_matches('\\').replace('\\', "/");
    if rel.is_empty() {
        return;
    }
    if let Some(after_app) = rel.strip_prefix("App/") {
        push_known(&format!("src/{after_app}.php"), path_index, out);
    }
    push_known(&format!("{rel}.php"), path_index, out);
    push_known(&format!("src/{rel}.php"), path_index, out);
}

fn resolve_dart_import(
    from_path: &str,
    spec: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    if let Some(pkg) = spec.strip_prefix("package:") {
        if let Some((_, rel)) = pkg.split_once('/') {
            push_known(&format!("lib/{rel}"), path_index, out);
        }
        return;
    }
    let joined = normalize_join(dir_of(from_path), spec);
    push_known(&joined, path_index, out);
}

fn resolve_mcfunction_call(spec: &str, path_index: &BTreeMap<&str, usize>, out: &mut Vec<usize>) {
    if let Some((ns, rel)) = spec.split_once(':') {
        push_known(
            &format!("data/{ns}/function/{rel}.mcfunction"),
            path_index,
            out,
        );
    }
}
