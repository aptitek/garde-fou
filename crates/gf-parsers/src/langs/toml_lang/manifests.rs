//! Supply-chain manifest audits (`TOML-UNPINNED-GIT-OR-WILDCARD` & `TOML-WORKSPACE-BYPASS`)
//! over `toml_edit` AST spans for `Cargo.toml`, `pyproject.toml`, and `mise.toml`.

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, RepoPath, Span, TomlManifestSummary, TomlViolation};
use toml_edit::{ImDocument, Item, Table, TableLike, Value};

use super::span_util::span_from_byte_offset;

const CARGO_DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

struct CargoAuditCtx<'a> {
    source: &'a str,
    collect_member_hardcoded: bool,
    summary: &'a mut TomlManifestSummary,
    out: &'a mut Vec<LangRuleFinding>,
}

fn item_span(source: &str, item: &Item, fallback: Span) -> Span {
    item.span()
        .map_or(fallback, |r| span_from_byte_offset(source, r.start))
}

fn has_non_empty_str(tbl: &dyn TableLike, key: &str) -> bool {
    tbl.get(key)
        .and_then(Item::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

impl CargoAuditCtx<'_> {
    fn check_dep_table(&mut self, dep_key: &str, dep_item: &Item, key_span: Span) {
        let Some(tbl) = dep_item.as_table_like() else {
            return;
        };
        let sp = item_span(self.source, dep_item, key_span);
        if let Some(ver_item) = tbl.get("version")
            && ver_item.as_str().is_some_and(|v| v.trim() == "*")
        {
            self.out.push(LangRuleFinding::new(
                TomlViolation::UnpinnedGitOrWildcard,
                item_span(self.source, ver_item, sp),
                format!("Cargo dependency `{dep_key}` uses wildcard `version = \"*\"`"),
            ));
        }
        if let Some(git_item) = tbl.get("git")
            && git_item.as_str().is_some()
            && !(has_non_empty_str(tbl, "rev") || has_non_empty_str(tbl, "tag"))
        {
            self.out.push(LangRuleFinding::new(
                TomlViolation::UnpinnedGitOrWildcard,
                item_span(self.source, git_item, sp),
                format!("Cargo `git` dependency `{dep_key}` lacks immutable `rev` or `tag`"),
            ));
        }
        let is_ws = tbl.get("workspace").and_then(Item::as_bool) == Some(true);
        if self.collect_member_hardcoded && !is_ws && tbl.get("path").is_none() {
            self.summary
                .hardcoded_member_deps
                .push((String::from(dep_key), key_span));
        }
    }

    fn check_dep_entry(&mut self, dep_key: &str, dep_item: &Item, key_span: Span) {
        if let Some(ver_str) = dep_item.as_str() {
            if ver_str.trim() == "*" {
                self.out.push(LangRuleFinding::new(
                    TomlViolation::UnpinnedGitOrWildcard,
                    item_span(self.source, dep_item, key_span),
                    format!("Cargo dependency `{dep_key}` uses wildcard version `\"*\"`"),
                ));
            }
            if self.collect_member_hardcoded {
                self.summary
                    .hardcoded_member_deps
                    .push((String::from(dep_key), key_span));
            }
        } else {
            self.check_dep_table(dep_key, dep_item, key_span);
        }
    }

    fn audit_dep_table(&mut self, tbl: &dyn TableLike) {
        for (dep_k, dep_v) in tbl.iter() {
            let key_sp = tbl.key(dep_k).and_then(toml_edit::Key::span).map_or_else(
                || item_span(self.source, dep_v, Span::FIRST),
                |r| span_from_byte_offset(self.source, r.start),
            );
            self.check_dep_entry(dep_k, dep_v, key_sp);
        }
    }

    fn audit_all_dep_sections(&mut self, container: &dyn TableLike) {
        for sec in CARGO_DEP_TABLES {
            if let Some(tbl) = container.get(sec).and_then(Item::as_table_like) {
                self.audit_dep_table(tbl);
            }
        }
    }
}

fn check_cargo_manifest(
    path: &RepoPath,
    source: &str,
    root: &Table,
    out: &mut Vec<LangRuleFinding>,
) -> TomlManifestSummary {
    let mut summary = TomlManifestSummary::default();
    let is_root = path.as_str() == "Cargo.toml";
    if let Some(ws) = root.get("workspace").and_then(Item::as_table_like) {
        summary.is_workspace_root = true;
        if let Some(ws_deps) = ws.get("dependencies").and_then(Item::as_table_like) {
            summary.workspace_deps = ws_deps
                .iter()
                .map(|(k, _)| String::from(k))
                .collect::<BTreeSet<_>>();
            CargoAuditCtx {
                source,
                collect_member_hardcoded: false,
                summary: &mut summary,
                out,
            }
            .audit_dep_table(ws_deps);
        }
    }
    let mut ctx = CargoAuditCtx {
        source,
        collect_member_hardcoded: !is_root,
        summary: &mut summary,
        out,
    };
    ctx.audit_all_dep_sections(root);
    if let Some(targets) = root.get("target").and_then(Item::as_table_like) {
        for (_, t_cfg) in targets.iter() {
            if let Some(t_tbl) = t_cfg.as_table_like() {
                ctx.audit_all_dep_sections(t_tbl);
            }
        }
    }
    summary
}

fn check_uv_sources(source: &str, root: &Table, out: &mut Vec<LangRuleFinding>) {
    let Some(uv_sources) = root
        .get("tool")
        .and_then(Item::as_table_like)
        .and_then(|t| t.get("uv"))
        .and_then(Item::as_table_like)
        .and_then(|u| u.get("sources"))
        .and_then(Item::as_table_like)
    else {
        return;
    };
    for (pkg, spec) in uv_sources.iter() {
        if let Some(tbl) = spec.as_table_like()
            && let Some(git_it) = tbl.get("git")
            && git_it.as_str().is_some()
            && !(has_non_empty_str(tbl, "rev") || has_non_empty_str(tbl, "tag"))
        {
            out.push(LangRuleFinding::new(
                TomlViolation::UnpinnedGitOrWildcard,
                item_span(source, git_it, Span::FIRST),
                format!("`[tool.uv.sources]` git dependency `{pkg}` lacks `rev` or `tag`"),
            ));
        }
    }
}

fn check_pep621_git_deps(source: &str, root: &Table, out: &mut Vec<LangRuleFinding>) {
    let Some(deps) = root
        .get("project")
        .and_then(Item::as_table_like)
        .and_then(|p| p.get("dependencies"))
        .and_then(Item::as_array)
    else {
        return;
    };
    for val in deps {
        if let Some(s) = val.as_str()
            && let Some((_, after_git)) = s.split_once("git+")
            && after_git
                .rsplit_once('@')
                .is_none_or(|(_, r)| r.trim().len() < 2)
        {
            let sp = val
                .span()
                .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
            out.push(LangRuleFinding::new(
                TomlViolation::UnpinnedGitOrWildcard,
                sp,
                format!("`[project].dependencies` git URL `{s}` lacks `@<rev-or-tag>`"),
            ));
        }
    }
}

fn check_mise_tools(source: &str, root: &Table, out: &mut Vec<LangRuleFinding>) {
    let Some(tools) = root.get("tools").and_then(Item::as_table_like) else {
        return;
    };
    for (tool_name, item) in tools.iter() {
        let ver_opt = match item {
            Item::Value(Value::String(s)) => Some(s.value().as_str()),
            Item::Value(Value::InlineTable(it)) => it.get("version").and_then(Value::as_str),
            Item::Table(t) => t.get("version").and_then(Item::as_str),
            _ => None,
        };
        if ver_opt.is_some_and(|v| v.trim().eq_ignore_ascii_case("latest")) {
            out.push(LangRuleFinding::new(
                TomlViolation::UnpinnedGitOrWildcard,
                item_span(source, item, Span::FIRST),
                format!("`mise.toml` tool `{tool_name}` uses unpinned `\"latest\"` version"),
            ));
        }
    }
}

/// Audits `Cargo.toml`, `pyproject.toml`, and `mise.toml` manifests on a parsed [`ImDocument`].
pub fn check_toml_manifests(
    path: &RepoPath,
    source: &str,
    im_doc: &ImDocument<&str>,
    out: &mut Vec<LangRuleFinding>,
) -> Option<TomlManifestSummary> {
    let lower_name = path.file_name().to_ascii_lowercase();
    let root = im_doc.as_table();
    if lower_name == "cargo.toml" {
        Some(check_cargo_manifest(path, source, root, out))
    } else {
        if lower_name == "pyproject.toml" {
            check_uv_sources(source, root, out);
            check_pep621_git_deps(source, root, out);
        } else if lower_name == "mise.toml" || lower_name == ".mise.toml" {
            check_mise_tools(source, root, out);
        }
        None
    }
}
