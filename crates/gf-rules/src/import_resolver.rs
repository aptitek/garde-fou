//! Pure `#![no_std]` cross-language workspace import resolver shared by Tarjan (`GF071`)
//! and architectural `[[boundaries]]` (`GF072`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec::Vec;

use gf_core::{Language, RepoPath};

pub use crate::rust_import_resolver::normalize_join;
use crate::rust_import_resolver::{dir_of, push_known, resolve_rust_import};

const TS_PROBES: &[&str] = &[
    "",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    "/index.ts",
    "/index.tsx",
    "/index.js",
];

fn resolve_ts_import(
    from_path: &str,
    spec: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let base = if spec.starts_with('.') {
        normalize_join(dir_of(from_path), spec)
    } else if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
        format!("src/{}", rest.trim_start_matches('/'))
    } else {
        return;
    };
    for suffix in TS_PROBES {
        push_known(&format!("{base}{suffix}"), path_index, out);
    }
}

fn resolve_py_import(
    from_path: &str,
    spec: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let clean = spec.split_whitespace().next().unwrap_or("").trim();
    let dots = clean.bytes().take_while(|&b| b == b'.').count();
    let tail = clean[dots..].replace('.', "/");
    let prefixes = if dots >= 1 {
        let mut cur = dir_of(from_path);
        for _ in 1..dots {
            cur = dir_of(cur);
        }
        alloc::vec![normalize_join(cur, &tail)]
    } else {
        alloc::vec![tail.clone(), format!("src/{tail}")]
    };
    for pfx in prefixes {
        push_known(&format!("{pfx}.py"), path_index, out);
        push_known(&format!("{pfx}/__init__.py"), path_index, out);
    }
}

/// Resolves an import specifier in `from_path` (`lang`) to target document indices.
#[must_use]
pub fn resolve_import_indices(
    from_path: &RepoPath,
    lang: Language,
    module_source: &str,
    path_index: &BTreeMap<&str, usize>,
) -> Vec<usize> {
    let mut out = Vec::new();
    let raw_from = from_path.as_str();
    match lang {
        Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx => {
            resolve_ts_import(raw_from, module_source, path_index, &mut out);
        }
        Language::Python => resolve_py_import(raw_from, module_source, path_index, &mut out),
        Language::Rust => resolve_rust_import(raw_from, module_source, path_index, &mut out),
        _ => {}
    }
    out.retain(|&idx| path_index.get(raw_from) != Some(&idx));
    out
}
