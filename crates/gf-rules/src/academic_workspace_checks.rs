//! Pure `#![no_std]` evaluators for Jupyter Notebook local cell/output rules (`NB-OVERSIZED-CELL`,
//! `NB-DIRTY-OUTPUTS`) and workspace-wide Quarto (`QUARTO-*`) & Obsidian (`OBS-*`) link integrity.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, NotebookViolation, ObsidianViolation, QuartoViolation, UastDocument,
    Validation, WorkspaceRefItem,
};

use crate::checks::{SourceLoc, rule_error};
use crate::rust_import_resolver::{dir_of, normalize_join};

/// Evaluates local Jupyter Notebook cell-size (`NB-OVERSIZED-CELL`) and clean-output
/// (`NB-DIRTY-OUTPUTS`) rules (`GF080`).
pub fn check_local_notebook_rules(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let Some(acad) = &doc.academic else {
        return Validation::Valid(());
    };
    let mut diags = Vec::new();
    let max_cell = config.limits.max_function_lines;
    for &(cell_idx, eff_lines, span) in &acad.notebook_cell_lines {
        if eff_lines > max_cell {
            let v = NotebookViolation::OversizedCell;
            diags.push(rule_error(
                "GF080",
                format!(
                    "[{}] [cell #{cell_idx}] code cell has {eff_lines} effective lines (max {max_cell})",
                    v.code()
                ),
                v.remediation(),
                SourceLoc::new(&doc.path, span),
            ));
        }
    }
    if config.notebook.require_clean_outputs {
        let v = NotebookViolation::DirtyOutputs;
        for &(cell_idx, span) in &acad.notebook_output_spans {
            diags.push(rule_error(
                "GF080",
                format!(
                    "[{}] [cell #{cell_idx}] notebook cell contains persisted outputs",
                    v.code()
                ),
                v.remediation(),
                SourceLoc::new(&doc.path, span),
            ));
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn resolve_rel_path(from_file: &str, raw_target: &str, known: &BTreeSet<&str>) -> bool {
    let clean = raw_target
        .split('#')
        .next()
        .unwrap_or(raw_target)
        .split('?')
        .next()
        .unwrap_or(raw_target)
        .trim();
    if clean.is_empty() || clean.starts_with("http://") || clean.starts_with("https://") {
        return true;
    }
    let trimmed = clean.trim_start_matches('/');
    if known.contains(trimmed) {
        return true;
    }
    let joined = normalize_join(dir_of(from_file), trimmed);
    known.contains(joined.as_str())
}

fn github_slug(heading: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in heading.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '_' {
            out.push(lower);
            prev_dash = false;
        } else if (lower.is_whitespace() || lower == '-') && !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    String::from(out.trim_end_matches('-'))
}

struct VaultIndex<'a> {
    known_paths: BTreeSet<&'a str>,
    by_stem: BTreeMap<String, Vec<&'a UastDocument>>,
    by_filename: BTreeMap<String, Vec<&'a UastDocument>>,
}

impl<'a> VaultIndex<'a> {
    fn build(docs: &'a [UastDocument]) -> Self {
        let mut known_paths = BTreeSet::new();
        let mut by_stem: BTreeMap<String, Vec<&'a UastDocument>> = BTreeMap::new();
        let mut by_filename: BTreeMap<String, Vec<&'a UastDocument>> = BTreeMap::new();
        for doc in docs {
            known_paths.insert(doc.path.as_str());
            let fname = doc.path.file_name().to_ascii_lowercase();
            by_filename.entry(fname.clone()).or_default().push(doc);
            if let Some(stem) = fname.strip_suffix(".md") {
                by_stem.entry(String::from(stem)).or_default().push(doc);
            }
        }
        Self {
            known_paths,
            by_stem,
            by_filename,
        }
    }

    fn target_matches_fragment(target_doc: &UastDocument, fragment: Option<&str>) -> bool {
        let Some(frag) = fragment.filter(|f| !f.trim().is_empty()) else {
            return true;
        };
        if let Some(block_id) = frag.strip_prefix('^') {
            return target_doc
                .academic
                .as_ref()
                .is_some_and(|a| a.obsidian_block_ids.contains(block_id));
        }
        let slug = github_slug(frag);
        target_doc
            .contenus
            .as_ref()
            .is_none_or(|c| c.heading_slugs.is_empty() || c.heading_slugs.contains(&slug))
    }

    fn resolve_wikilink(&self, from_doc: &UastDocument, item: &WorkspaceRefItem) -> bool {
        let target = item.target.trim();
        if target.is_empty() {
            return Self::target_matches_fragment(from_doc, item.fragment.as_deref());
        }
        let lower = target.to_ascii_lowercase();
        let lower_no_md = lower.strip_suffix(".md").unwrap_or(&lower);
        if self.known_paths.contains(target)
            || self.known_paths.contains(format!("{target}.md").as_str())
        {
            return true;
        }
        if let Some(matches) = self.by_stem.get(lower_no_md) {
            return matches
                .iter()
                .any(|d| Self::target_matches_fragment(d, item.fragment.as_deref()));
        }
        if let Some(matches) = self.by_filename.get(&lower) {
            return !matches.is_empty();
        }
        let stem_last = lower_no_md.rsplit('/').next().unwrap_or(lower_no_md);
        self.by_stem.get(stem_last).is_some_and(|matches| {
            matches
                .iter()
                .any(|d| Self::target_matches_fragment(d, item.fragment.as_deref()))
        })
    }
}

#[derive(Clone, Copy)]
struct RefCheckSpec<'a> {
    items: &'a [WorkspaceRefItem],
    code: &'static str,
    label: &'static str,
    remediation: &'static str,
}

fn check_rel_refs(
    doc: &UastDocument,
    known: &BTreeSet<&str>,
    spec: RefCheckSpec<'_>,
    out: &mut Vec<Diagnostic>,
) {
    for item in spec.items {
        if !resolve_rel_path(doc.path.as_str(), &item.target, known) {
            out.push(rule_error(
                "GF080",
                format!("[{}] {} `{}`", spec.code, spec.label, item.target),
                spec.remediation,
                SourceLoc::new(&doc.path, item.span),
            ));
        }
    }
}

fn check_doc_academic_refs(doc: &UastDocument, vault: &VaultIndex<'_>, out: &mut Vec<Diagnostic>) {
    let Some(acad) = &doc.academic else {
        return;
    };
    let specs = [
        RefCheckSpec {
            items: &acad.quarto_config_refs,
            code: QuartoViolation::MissingReferencedFile.code(),
            label: "`_quarto.yml` references missing file",
            remediation: QuartoViolation::MissingReferencedFile.remediation(),
        },
        RefCheckSpec {
            items: &acad.quarto_shortcode_refs,
            code: QuartoViolation::BrokenShortcodeTarget.code(),
            label: "Quarto shortcode references missing target",
            remediation: QuartoViolation::BrokenShortcodeTarget.remediation(),
        },
        RefCheckSpec {
            items: &acad.canvas_file_refs,
            code: ObsidianViolation::CanvasBrokenRef.code(),
            label: "`.canvas` file node references missing file",
            remediation: ObsidianViolation::CanvasBrokenRef.remediation(),
        },
    ];
    for spec in specs {
        check_rel_refs(doc, &vault.known_paths, spec, out);
    }
    let obs_w = ObsidianViolation::BrokenWikilinkOrEmbed;
    for item in &acad.obsidian_wikilinks {
        if !vault.resolve_wikilink(doc, item) {
            out.push(rule_error(
                "GF080",
                format!(
                    "[{}] unresolved Obsidian wikilink/embed `[[{}]]`",
                    obs_w.code(),
                    item.target
                ),
                obs_w.remediation(),
                SourceLoc::new(&doc.path, item.span),
            ));
        }
    }
}

/// Evaluates workspace-wide Quarto (`QUARTO-*`) and Obsidian (`OBS-*`) reference integrity.
pub fn check_workspace_academic(docs: &[UastDocument]) -> Validation<(), Diagnostic> {
    let has_any = docs.iter().any(|d| d.academic.is_some());
    if !has_any {
        return Validation::Valid(());
    }
    let vault = VaultIndex::build(docs);
    let mut diags = Vec::new();
    for doc in docs {
        check_doc_academic_refs(doc, &vault, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
