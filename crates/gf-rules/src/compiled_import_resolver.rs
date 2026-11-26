//! Pure `#![no_std]` workspace import resolver & Go structural interface matcher (`GF071`/`GF072`/`GF075`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{Language, UastDocument};

/// Pre-indexed Go module/package, Kotlin `package`, and C# `namespace` lookup tables.
pub struct CompiledWorkspaceIndex {
    go_module_path: Option<String>,
    go_dir_files: BTreeMap<String, Vec<usize>>,
    kotlin_packages: BTreeMap<String, Vec<usize>>,
    csharp_namespaces: BTreeMap<String, Vec<usize>>,
}

impl CompiledWorkspaceIndex {
    /// Builds deterministic package/namespace indices across `docs`.
    #[must_use]
    pub fn from_docs(docs: &[UastDocument]) -> Self {
        let mut index = Self {
            go_module_path: None,
            go_dir_files: BTreeMap::new(),
            kotlin_packages: BTreeMap::new(),
            csharp_namespaces: BTreeMap::new(),
        };
        for (idx, doc) in docs.iter().enumerate() {
            index.index_doc(idx, doc);
        }
        index
    }

    fn index_doc(&mut self, idx: usize, doc: &UastDocument) {
        if let Some(meta) = &doc.compiled_meta {
            if let Some(m) = &meta.go_module_path {
                self.go_module_path = Some(m.clone());
            }
            if let Some(pkg) = &meta.package_or_namespace {
                match doc.language {
                    Language::Kotlin => self
                        .kotlin_packages
                        .entry(pkg.clone())
                        .or_default()
                        .push(idx),
                    Language::CSharp => self
                        .csharp_namespaces
                        .entry(pkg.clone())
                        .or_default()
                        .push(idx),
                    _ => {}
                }
            }
        }
        if doc.language == Language::Go && !doc.path.as_str().ends_with("_test.go") {
            let dir = doc.path.as_str().rsplit_once('/').map_or("", |(d, _)| d);
            self.go_dir_files
                .entry(String::from(dir))
                .or_default()
                .push(idx);
        }
    }

    /// Resolves a Go `import "module/pkg/sub"` to non-test `.go` files in `pkg/sub`.
    pub fn resolve_go(&self, module_source: &str, out: &mut Vec<usize>) {
        let clean = module_source.trim().trim_matches('"');
        let rel_dir = if let Some(mod_root) = &self.go_module_path {
            if clean == mod_root {
                ""
            } else if let Some(rest) = clean
                .strip_prefix(mod_root.as_str())
                .and_then(|s| s.strip_prefix('/'))
            {
                rest
            } else {
                return;
            }
        } else {
            clean
        };
        if let Some(indices) = self.go_dir_files.get(rel_dir) {
            out.extend_from_slice(indices);
        }
    }

    /// Resolves a Kotlin `import a.b.c.Foo` or `import a.b.c.*` to `.kt` files declaring `package a.b.c`.
    pub fn resolve_kotlin(&self, module_source: &str, out: &mut Vec<usize>) {
        let clean = module_source.trim().trim_end_matches(".*");
        if let Some(indices) = self.kotlin_packages.get(clean) {
            out.extend_from_slice(indices);
            return;
        }
        if let Some((pkg, _)) = clean.rsplit_once('.')
            && let Some(indices) = self.kotlin_packages.get(pkg)
        {
            out.extend_from_slice(indices);
        }
    }

    /// Resolves a C# `using A.B.C;` directive to `.cs` files declaring `namespace A.B.C`.
    pub fn resolve_csharp(&self, module_source: &str, out: &mut Vec<usize>) {
        let clean = module_source.trim().trim_end_matches(';');
        if let Some(indices) = self.csharp_namespaces.get(clean) {
            out.extend_from_slice(indices);
        }
    }
}

fn collect_go_concrete_methods(docs: &[UastDocument]) -> BTreeMap<&str, BTreeSet<&str>> {
    let mut out: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for meta in docs.iter().filter_map(|d| d.compiled_meta.as_ref()) {
        for ct in &meta.go_concrete_types {
            let entry = out.entry(ct.type_name.as_str()).or_default();
            entry.extend(ct.methods.iter().map(String::as_str));
        }
    }
    out
}

fn match_go_iface<'a>(
    iface: &'a gf_core::StructuralInterfaceDecl,
    concrete: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    impl_counts: &mut BTreeMap<&'a str, BTreeSet<&'a str>>,
) {
    if iface.methods.is_empty() {
        return;
    }
    for (&type_name, methods) in concrete {
        if iface
            .methods
            .iter()
            .all(|req| methods.contains(req.as_str()))
        {
            impl_counts
                .entry(iface.name.as_str())
                .or_default()
                .insert(type_name);
        }
    }
}

/// Evaluates implicit Go interface satisfaction (`GF075`): for each non-empty Go interface `I`,
/// any concrete type `T` whose method set is a superset of `I`'s methods counts as an implementor.
pub fn augment_go_structural_impls<'a>(
    docs: &'a [UastDocument],
    impl_counts: &mut BTreeMap<&'a str, BTreeSet<&'a str>>,
) {
    let concrete_methods = collect_go_concrete_methods(docs);
    for meta in docs.iter().filter_map(|d| d.compiled_meta.as_ref()) {
        for iface in &meta.go_interfaces {
            match_go_iface(iface, &concrete_methods, impl_counts);
        }
    }
}
