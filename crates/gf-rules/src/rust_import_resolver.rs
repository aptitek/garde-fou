//! Pure `#![no_std]` Rust `use` / inline-path resolver supporting arbitrary module depth
//! (`crate::a::b::c::d`), chained `super::`, and Rust 2018+ `foo.rs` + `foo/bar.rs` layouts.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub fn dir_of(path: &str) -> &str {
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
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}

fn push_unique(idx: usize, out: &mut Vec<usize>) {
    if !out.contains(&idx) {
        out.push(idx);
    }
}

pub fn push_known(cand: &str, path_index: &BTreeMap<&str, usize>, out: &mut Vec<usize>) {
    if let Some(&idx) = path_index.get(cand) {
        push_unique(idx, out);
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

fn rust_own_module_dir(from_path: &str) -> String {
    if from_path.ends_with("/lib.rs")
        || from_path.ends_with("/main.rs")
        || from_path.ends_with("/mod.rs")
        || matches!(from_path, "lib.rs" | "main.rs" | "mod.rs")
    {
        String::from(dir_of(from_path))
    } else {
        String::from(from_path.strip_suffix(".rs").unwrap_or(from_path))
    }
}

fn lookup_rust_mod_hits(dir: &str, seg: &str, path_index: &BTreeMap<&str, usize>) -> Vec<usize> {
    let mut hits = Vec::new();
    for cand in [
        normalize_join(dir, &format!("{seg}.rs")),
        normalize_join(dir, &format!("{seg}/mod.rs")),
    ] {
        if let Some(&idx) = path_index.get(cand.as_str()) {
            push_unique(idx, &mut hits);
        }
    }
    hits
}

fn walk_rust_segments_deepest(
    start_dir: &str,
    segments: &[&str],
    path_index: &BTreeMap<&str, usize>,
) -> Vec<usize> {
    let mut cur_dir = String::from(start_dir);
    let mut deepest: Vec<usize> = Vec::new();
    for &seg in segments {
        let hits = lookup_rust_mod_hits(&cur_dir, seg, path_index);
        if !hits.is_empty() {
            deepest = hits;
        }
        cur_dir = normalize_join(&cur_dir, seg);
    }
    deepest
}

fn owner_files_of_mod_dir(
    mod_dir: &str,
    crate_src: &str,
    path_index: &BTreeMap<&str, usize>,
) -> Vec<usize> {
    let mut hits = Vec::new();
    if mod_dir == crate_src {
        push_known(&normalize_join(crate_src, "lib.rs"), path_index, &mut hits);
        return hits;
    }
    push_known(&format!("{mod_dir}.rs"), path_index, &mut hits);
    push_known(&format!("{mod_dir}/mod.rs"), path_index, &mut hits);
    hits
}

fn resolve_crate_use(
    from_path: &str,
    parts: &[&str],
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let crate_src = rust_crate_src_dir(from_path);
    let deepest = walk_rust_segments_deepest(crate_src, &parts[1..], path_index);
    if !deepest.is_empty() {
        for idx in deepest {
            push_unique(idx, out);
        }
        return;
    }
    let lib_cand = normalize_join(crate_src, "lib.rs");
    if from_path != lib_cand {
        push_known(&lib_cand, path_index, out);
    }
}

fn resolve_super_use(
    from_path: &str,
    segs: &[&str],
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    let crate_src = rust_crate_src_dir(from_path);
    let super_count = segs.iter().take_while(|&&s| s == "super").count();
    let mut target_dir = rust_own_module_dir(from_path);
    for _ in 0..super_count {
        target_dir = String::from(dir_of(&target_dir));
    }
    let deepest = walk_rust_segments_deepest(&target_dir, &segs[super_count..], path_index);
    let resolved = if deepest.is_empty() {
        owner_files_of_mod_dir(&target_dir, crate_src, path_index)
    } else {
        deepest
    };
    for idx in resolved {
        push_unique(idx, out);
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
        "super" => resolve_super_use(from_path, &segs, path_index, out),
        "self" => {
            let own_dir = rust_own_module_dir(from_path);
            for idx in walk_rust_segments_deepest(&own_dir, &segs[1..], path_index) {
                push_unique(idx, out);
            }
        }
        other => {
            let own_dir = rust_own_module_dir(from_path);
            let mut hits = walk_rust_segments_deepest(&own_dir, &segs, path_index);
            if hits.is_empty() {
                hits = walk_rust_segments_deepest(dir_of(from_path), &segs, path_index);
            }
            for idx in hits {
                push_unique(idx, out);
            }
            let kebab = other.replace('_', "-");
            push_known(&format!("crates/{kebab}/src/lib.rs"), path_index, out);
            push_known(&format!("crates/{other}/src/lib.rs"), path_index, out);
        }
    }
}

pub fn resolve_rust_import(
    raw_from: &str,
    module_source: &str,
    path_index: &BTreeMap<&str, usize>,
    out: &mut Vec<usize>,
) {
    for expanded in expand_rust_use_roots(module_source) {
        resolve_single_rust_path(raw_from, &expanded, path_index, out);
    }
}
