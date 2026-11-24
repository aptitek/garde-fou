//! Pure `#![no_std]` cross-language workspace import resolver shared by Tarjan (`GF071`)
//! and architectural `[[boundaries]]` (`GF072`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{Language, RepoPath};

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

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Normalizes a relative path join (`base_dir` + `rel`), collapsing `.` and `..`.
#[must_use]
pub fn normalize_join(base_dir: &str, rel: &str) -> String {
    let raw = if rel.starts_with('/') {
        String::from(rel.trim_start_matches('/'))
    } else if base_dir.is_empty() {
        String::from(rel)
    } else {
        format!("{base_dir}/{rel}")
    };
    let mut stack: Vec<&str> = Vec::new();
    for seg in raw.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                let _ = stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}

fn push_known(cand: &str, path_index: &BTreeMap<&str, usize>, out: &mut Vec<usize>) -> bool {
    let Some(&idx) = path_index.get(cand) else {
        return false;
    };
    if !out.contains(&idx) {
        out.push(idx);
    }
    true
}

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
        let _ = push_known(&format!("{base}{suffix}"), path_index, out);
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
        let _ = push_known(&format!("{pfx}.py"), path_index, out);
        let _ = push_known(&format!("{pfx}/__init__.py"), path_index, out);
    }
}

fn expand_rust_use_roots(raw: &str) -> Vec<String> {
    let trimmed = raw.trim().trim_end_matches(';');
    let Some((prefix, rest)) = trimmed.split_once('{') else {
        return alloc::vec![String::from(trimmed)];
    };
    let Some((inner, _)) = rest.rsplit_once('}') else {
        return alloc::vec![String::from(trimmed)];
    };
    let base = prefix.trim().trim_end_matches("::");
    inner
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "self")
        .map(|item| {
            if base.is_empty() {
                String::from(item)
            } else {
                format!("{base}::{item}")
            }
        })
        .collect()
}

fn rust_crate_src_dir(from_path: &str) -> &str {
    if let Some((before_src, _)) = from_path.split_once("/src/") {
        return &from_path[..before_src.len() + 4];
    }
    if from_path.starts_with("src/") {
        return "src";
    }
    dir_of(from_path)
}

fn probe_rust_mod(
    dir: &str,
    seg: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) -> bool {
    let file_hit = push_known(&normalize_join(dir, &format!("{seg}.rs")), path_index, out);
    let mod_hit = push_known(
        &normalize_join(dir, &format!("{seg}/mod.rs")),
        path_index,
        out,
    );
    file_hit || mod_hit
}

fn resolve_crate_use(
    from_path: &str,
    parts: &[&str],
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let Some(&first_mod) = parts.get(1) else {
        return;
    };
    let crate_src = rust_crate_src_dir(from_path);
    let mut matched = probe_rust_mod(crate_src, first_mod, path_index, out);
    if let Some(&sub_mod) = parts.get(2) {
        matched |= probe_rust_mod(
            &format!("{crate_src}/{first_mod}"),
            sub_mod,
            path_index,
            out,
        );
    }
    let lib_cand = format!("{crate_src}/lib.rs");
    if !matched && from_path != lib_cand {
        let _ = push_known(&lib_cand, path_index, out);
    }
}

fn resolve_single_rust_path(
    from_path: &str,
    use_path: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let segs: Vec<&str> = use_path
        .split("::")
        .map(|s| s.split_whitespace().next().unwrap_or(""))
        .filter(|s| !s.is_empty())
        .collect();
    let Some(&first) = segs.first() else {
        return;
    };
    match first {
        "crate" => resolve_crate_use(from_path, &segs, path_index, out),
        "super" => {
            let parent = if from_path.ends_with("/mod.rs") {
                dir_of(dir_of(from_path))
            } else {
                dir_of(from_path)
            };
            if let Some(&seg1) = segs.get(1) {
                let _ = probe_rust_mod(parent, seg1, path_index, out);
            }
        }
        other => {
            let cur_dir = dir_of(from_path);
            let _ = probe_rust_mod(cur_dir, other, path_index, out);
            if let Some(&seg1) = segs.get(1) {
                let _ = probe_rust_mod(&format!("{cur_dir}/{other}"), seg1, path_index, out);
            }
            let kebab = other.replace('_', "-");
            let _ = push_known(&format!("crates/{kebab}/src/lib.rs"), path_index, out);
            let _ = push_known(&format!("crates/{other}/src/lib.rs"), path_index, out);
        }
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
        Language::Rust => {
            for expanded in expand_rust_use_roots(module_source) {
                resolve_single_rust_path(raw_from, &expanded, path_index, &mut out);
            }
        }
        _ => {}
    }
    out.retain(|&idx| path_index.get(raw_from) != Some(&idx));
    out
}
