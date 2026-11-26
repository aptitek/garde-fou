//! `_quarto.yml` / `_quarto.yaml` AST reference extractor (`QUARTO-MISSING-REFERENCED-FILE`).

use gf_core::{RepoPath, Span, WorkspaceRefItem};

use crate::langs::yaml::anchors_merge::ResolvedYaml;

/// Returns `true` when `path` is a Quarto project configuration file (`_quarto.yml` / `_quarto.yaml`).
#[must_use]
pub fn is_quarto_config_path(path: &RepoPath) -> bool {
    let fname = path.file_name();
    fname.eq_ignore_ascii_case("_quarto.yml") || fname.eq_ignore_ascii_case("_quarto.yaml")
}

fn push_ref_if_local(raw: &str, span: Span, out: &mut Vec<WorkspaceRefItem>) {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with('#')
        || trimmed.contains('*')
    {
        return;
    }
    out.push(WorkspaceRefItem {
        target: String::from(trimmed),
        fragment: None,
        span,
    });
}

fn is_quarto_doc_target(val: &str) -> bool {
    [".qmd", ".md", ".ipynb", ".html"]
        .iter()
        .any(|ext| val.to_ascii_lowercase().ends_with(ext))
}

fn collect_mapping_nav_refs(
    entries: &[(crate::langs::yaml::yaml_ast::YamlScalar, ResolvedYaml)],
    out: &mut Vec<WorkspaceRefItem>,
) {
    for (k, v) in entries {
        match k.value.as_str() {
            "href" | "file" | "part" => {
                if let ResolvedYaml::Scalar(s) = v
                    && is_quarto_doc_target(&s.value)
                {
                    push_ref_if_local(&s.value, s.span, out);
                }
            }
            "chapters" | "contents" | "menu" | "left" | "right" | "appendices" => {
                collect_nav_or_chapter_refs(v, out);
            }
            _ => {}
        }
    }
}

fn collect_nav_or_chapter_refs(node: &ResolvedYaml, out: &mut Vec<WorkspaceRefItem>) {
    match node {
        ResolvedYaml::Scalar(s) => push_ref_if_local(&s.value, s.span, out),
        ResolvedYaml::Sequence { items, .. } => {
            for item in items {
                collect_nav_or_chapter_refs(item, out);
            }
        }
        ResolvedYaml::Mapping { entries, .. } => collect_mapping_nav_refs(entries, out),
    }
}

fn is_local_theme_path(v: &str) -> bool {
    v.contains('/')
        || std::path::Path::new(v)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("scss") || e.eq_ignore_ascii_case("css"))
}

fn collect_theme_refs(node: &ResolvedYaml, out: &mut Vec<WorkspaceRefItem>) {
    match node {
        ResolvedYaml::Scalar(s) => {
            let v = s.value.trim();
            if is_local_theme_path(v) {
                push_ref_if_local(v, s.span, out);
            }
        }
        ResolvedYaml::Sequence { items, .. } => {
            for item in items {
                collect_theme_refs(item, out);
            }
        }
        ResolvedYaml::Mapping { entries, .. } => {
            for (_, v) in entries {
                collect_theme_refs(v, out);
            }
        }
    }
}

fn collect_format_themes(node: &ResolvedYaml, out: &mut Vec<WorkspaceRefItem>) {
    let ResolvedYaml::Mapping { entries, .. } = node else {
        return;
    };
    for (_, fmt_val) in entries {
        if let Some(theme) = fmt_val.get("theme") {
            collect_theme_refs(theme, out);
        }
        if let Some(css) = fmt_val.get("css") {
            collect_theme_refs(css, out);
        }
    }
}

fn collect_section_keys(
    root: &ResolvedYaml,
    section: &str,
    keys: &[&str],
    refs: &mut Vec<WorkspaceRefItem>,
) {
    let Some(node) = root.get(section) else {
        return;
    };
    for &key in keys {
        if let Some(val) = node.get(key) {
            collect_nav_or_chapter_refs(val, refs);
        }
    }
}

/// Extracts all local file references declared in a resolved `_quarto.yml` document.
#[must_use]
pub fn collect_quarto_config_refs(resolved: &ResolvedYaml) -> Vec<WorkspaceRefItem> {
    let mut refs = Vec::new();
    collect_section_keys(resolved, "project", &["render"], &mut refs);
    collect_section_keys(resolved, "book", &["chapters", "appendices"], &mut refs);
    collect_section_keys(resolved, "website", &["sidebar", "navbar"], &mut refs);
    for key in ["bibliography", "csl"] {
        if let Some(val) = resolved.get(key) {
            collect_nav_or_chapter_refs(val, &mut refs);
        }
    }
    if let Some(fmt) = resolved.get("format") {
        collect_format_themes(fmt, &mut refs);
    }
    refs
}
