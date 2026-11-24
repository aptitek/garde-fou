//! Pure `#![no_std]` TypeScript / JavaScript ESM import resolver (`GF071` & `GF072`):
//! `.js`/`.jsx`/`.mjs`/`.cjs` -> `.ts`/`.tsx`/`.d.ts` remapping, implicit `index`,
//! `@/` & `~/` aliases, and `tsconfig.json` `baseUrl` + `paths` mapping.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::UastDocument;

use crate::rust_import_resolver::{dir_of, normalize_join, push_known};

const EXTENSIONLESS_TS_PROBES: &[&str] = &[
    "",
    ".ts",
    ".tsx",
    ".d.ts",
    ".mts",
    ".cts",
    ".js",
    ".jsx",
    ".mjs",
    ".cjs",
    "/index.ts",
    "/index.tsx",
    "/index.d.ts",
    "/index.js",
    "/index.jsx",
];

const ESM_REMAPPINGS: &[(&str, &[&str])] = &[
    (".jsx", &[".tsx", ".ts"]),
    (".mjs", &[".mts", ".d.mts", ".ts"]),
    (".cjs", &[".cts", ".d.cts", ".ts"]),
    (".js", &[".ts", ".tsx", ".d.ts"]),
];

struct TsProjectAliases {
    base_dir: String,
    has_base_url: bool,
    paths: Vec<(String, Vec<String>)>,
}

/// Pre-indexed workspace import context shared across `GF071` and `GF072`.
pub struct TsWorkspaceAliases {
    projects: Vec<TsProjectAliases>,
}

impl TsWorkspaceAliases {
    /// Extracts `baseUrl` and `paths` tables from all `tsconfig*.json` documents in `docs`.
    #[must_use]
    pub fn from_docs(docs: &[UastDocument]) -> Self {
        let mut projects = Vec::new();
        for doc in docs {
            let Some(ts) = &doc.tsconfig else {
                continue;
            };
            let cfg_dir = dir_of(doc.path.as_str());
            let base_dir = ts
                .base_url
                .as_deref()
                .map_or_else(|| String::from(cfg_dir), |b| normalize_join(cfg_dir, b));
            let paths = ts
                .paths
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            projects.push(TsProjectAliases {
                base_dir,
                has_base_url: ts.base_url.is_some(),
                paths,
            });
        }
        Self { projects }
    }

    /// Resolves a TypeScript/JavaScript module specifier to known workspace document indices.
    #[must_use]
    pub fn resolve_into(
        &self,
        from_path: &str,
        spec: &str,
        path_index: &BTreeMap<&str, usize>,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        let trimmed = spec.trim();
        if !trimmed.is_empty() {
            for base in collect_candidate_bases(from_path, trimmed, self) {
                probe_ts_base(&base, path_index, &mut out);
            }
        }
        out
    }
}

fn match_tsconfig_pattern<'a>(pattern: &str, spec: &'a str) -> Option<&'a str> {
    if let Some((pfx, sfx)) = pattern.split_once('*') {
        let after_pfx = spec.strip_prefix(pfx)?;
        after_pfx.strip_suffix(sfx)
    } else {
        (pattern == spec).then_some("")
    }
}

fn expand_tsconfig_target(base_dir: &str, target_tpl: &str, wildcard: &str) -> String {
    let substituted = if target_tpl.contains('*') {
        target_tpl.replace('*', wildcard)
    } else {
        String::from(target_tpl)
    };
    let clean = substituted.trim_start_matches("./");
    if base_dir.is_empty() {
        String::from(clean)
    } else {
        normalize_join(base_dir, clean)
    }
}

fn probe_ts_base(base: &str, path_index: &BTreeMap<&str, usize>, out: &mut Vec<usize>) {
    let before = out.len();
    push_known(base, path_index, out);
    if out.len() > before {
        return;
    }
    for &(ext, replacements) in ESM_REMAPPINGS {
        if let Some(stem) = base.strip_suffix(ext) {
            for &ts_ext in replacements {
                push_known(&format!("{stem}{ts_ext}"), path_index, out);
            }
            return;
        }
    }
    for &suffix in EXTENSIONLESS_TS_PROBES {
        push_known(&format!("{base}{suffix}"), path_index, out);
    }
}

fn append_project_alias_bases(proj: &TsProjectAliases, spec: &str, bases: &mut Vec<String>) {
    for (pat, targets) in &proj.paths {
        if let Some(captured) = match_tsconfig_pattern(pat, spec) {
            for tpl in targets {
                bases.push(expand_tsconfig_target(&proj.base_dir, tpl, captured));
            }
        }
    }
    if proj.has_base_url && !spec.starts_with('@') {
        bases.push(expand_tsconfig_target(&proj.base_dir, spec, ""));
    }
}

fn collect_candidate_bases(
    from_path: &str,
    spec: &str,
    aliases: &TsWorkspaceAliases,
) -> Vec<String> {
    let mut bases = Vec::new();
    if spec.starts_with('.') {
        bases.push(normalize_join(dir_of(from_path), spec));
        return bases;
    }
    if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
        let clean = rest.trim_start_matches('/');
        bases.push(format!("src/{clean}"));
        bases.push(format!("app/{clean}"));
    }
    for proj in &aliases.projects {
        append_project_alias_bases(proj, spec, &mut bases);
    }
    bases
}
