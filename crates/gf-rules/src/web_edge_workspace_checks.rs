//! Pure `#![no_std]` cross-file evaluators for Cloudflare Workers binding coherence (`CF-BINDING-COHERENCE`),
//! `wrangler.*` `nodejs_compat` requirement (`CF-WRANGLER-CONFIG`), Drizzle ORM dialect alignment
//! (`DRIZZLE-DIALECT-MISMATCH`), and `Forme` `@media (prefers-reduced-motion: reduce)` (`A11Y-MEDIA-FORM-MOTION`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::vec::Vec;

use gf_core::{
    A11yViolation, CloudflareViolation, Config, Diagnostic, DomainKind, DrizzleViolation,
    LangRuleViolation, RepoPath, Span, SqlDialectKind, ToolConfigViolation, UastDocument,
    Validation, WebEdgeConfigIssue, WebEdgeViolation, WranglerSummary, classify_path,
};

use crate::checks::{SourceLoc, rule_error};

fn doc_already_has_code_on_line(doc: &UastDocument, code: &str, span: Span) -> bool {
    doc.lang_rule_findings
        .iter()
        .any(|f| f.code() == code && f.span == span)
}

struct WranglerWorkspaceIndex<'a> {
    wrangler_docs: Vec<(&'a RepoPath, &'a WranglerSummary)>,
    known_bindings: BTreeSet<&'a str>,
    accessed_bindings: BTreeSet<&'a str>,
    has_code_env_access: bool,
}

fn collect_wrangler_index(docs: &[UastDocument]) -> WranglerWorkspaceIndex<'_> {
    let mut idx = WranglerWorkspaceIndex {
        wrangler_docs: Vec::new(),
        known_bindings: BTreeSet::new(),
        accessed_bindings: BTreeSet::new(),
        has_code_env_access: false,
    };
    for doc in docs {
        let Some(we) = &doc.web_edge else {
            continue;
        };
        if let Some(wr) = &we.wrangler {
            idx.wrangler_docs.push((&doc.path, wr));
            for b in &wr.declared_bindings {
                idx.known_bindings.insert(b.as_str());
            }
        }
        for k in &we.documented_env_keys {
            idx.known_bindings.insert(k.as_str());
        }
        for (acc, _) in &we.env_bindings_accessed {
            idx.accessed_bindings.insert(acc.as_str());
            idx.has_code_env_access = true;
        }
    }
    idx
}

fn check_node_colon_without_compat(
    doc: &UastDocument,
    wrangler_docs: &[(&RepoPath, &WranglerSummary)],
    out: &mut Vec<Diagnostic>,
) {
    let Some(we) = &doc.web_edge else {
        return;
    };
    let cf_node = CloudflareViolation::ForbiddenNodeApi;
    for (spec, span) in &we.node_colon_imports {
        if !doc_already_has_code_on_line(doc, cf_node.code(), *span) {
            out.push(rule_error(
                "GF080",
                format!(
                    "[{}] importing `{spec}` requires `\"nodejs_compat\"` in `wrangler.*` `compatibility_flags`",
                    cf_node.code()
                ),
                cf_node.remediation(),
                SourceLoc::new(&doc.path, *span),
            ));
        }
    }
    if !we.node_colon_imports.is_empty() {
        let v = ToolConfigViolation::WebEdge(WebEdgeConfigIssue::WranglerMissingNodejsCompat);
        for (w_path, wr) in wrangler_docs {
            out.push(rule_error(
                "GF081",
                format!(
                    "[{v}] [CF-WRANGLER-CONFIG] project imports `node:*` modules but `{w_path}` omits `\"nodejs_compat\"` in `compatibility_flags`"
                ),
                v.remediation(),
                SourceLoc::new(w_path, wr.span),
            ));
        }
    }
}

fn check_doc_undeclared_bindings(
    doc: &UastDocument,
    known_bindings: &BTreeSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(we) = &doc.web_edge else {
        return;
    };
    let cf_bind = CloudflareViolation::BindingCoherence;
    for (binding, span) in &we.env_bindings_accessed {
        if !known_bindings.contains(binding.as_str())
            && !doc_already_has_code_on_line(doc, cf_bind.code(), *span)
        {
            out.push(rule_error(
                "GF080",
                format!(
                    "[{}] Cloudflare Worker accesses undeclared binding `env.{binding}` not defined in `wrangler.*`",
                    cf_bind.code()
                ),
                cf_bind.remediation(),
                SourceLoc::new(&doc.path, *span),
            ));
        }
    }
}

fn check_unused_d1_r2_bindings(idx: &WranglerWorkspaceIndex<'_>, out: &mut Vec<Diagnostic>) {
    if !idx.has_code_env_access {
        return;
    }
    let cf_bind = CloudflareViolation::BindingCoherence;
    for (w_path, wr) in &idx.wrangler_docs {
        for (d1_r2_name, span) in &wr.d1_r2_bindings {
            if !idx.accessed_bindings.contains(d1_r2_name.as_str()) {
                out.push(rule_error(
                    "GF080",
                    format!(
                        "[{}] D1/R2 binding `{d1_r2_name}` declared in `{w_path}` is never used in `Code`",
                        cf_bind.code()
                    ),
                    cf_bind.remediation(),
                    SourceLoc::new(w_path, *span),
                ));
            }
        }
    }
}

fn check_wrangler_and_bindings(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let idx = collect_wrangler_index(docs);
    if idx.wrangler_docs.is_empty() {
        return;
    }
    let any_nodejs_compat = idx.wrangler_docs.iter().any(|(_, w)| w.has_nodejs_compat);
    for doc in docs {
        check_doc_undeclared_bindings(doc, &idx.known_bindings, out);
        if !any_nodejs_compat {
            check_node_colon_without_compat(doc, &idx.wrangler_docs, out);
        }
    }
    check_unused_d1_r2_bindings(&idx, out);
}

fn resolve_workspace_drizzle_dialect(docs: &[UastDocument]) -> Option<SqlDialectKind> {
    for doc in docs {
        let Some(we) = &doc.web_edge else {
            continue;
        };
        if let Some(d) = we.configured_drizzle_dialect {
            return Some(d);
        }
        if we.wrangler.as_ref().is_some_and(|w| w.has_d1) {
            return Some(SqlDialectKind::CloudflareD1);
        }
    }
    None
}

fn dialects_compatible(ws_dialect: SqlDialectKind, imported_core: SqlDialectKind) -> bool {
    match ws_dialect {
        SqlDialectKind::Sqlite | SqlDialectKind::CloudflareD1 => {
            matches!(
                imported_core,
                SqlDialectKind::Sqlite | SqlDialectKind::CloudflareD1
            )
        }
        other => other == imported_core,
    }
}

fn check_drizzle_workspace_dialects(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let Some(ws_dialect) = resolve_workspace_drizzle_dialect(docs) else {
        return;
    };
    let v = DrizzleViolation::DialectMismatch;
    for doc in docs {
        let Some(we) = &doc.web_edge else { continue };
        for &(imp_dialect, span) in &we.drizzle_dialect_imports {
            if !dialects_compatible(ws_dialect, imp_dialect)
                && !doc_already_has_code_on_line(doc, v.code(), span)
            {
                out.push(rule_error(
                    "GF080",
                    format!("[{}] `{}` imports `{imp_dialect}` Drizzle core in a `{ws_dialect}` workspace", v.code(), doc.path),
                    v.remediation(),
                    SourceLoc::new(&doc.path, span),
                ));
            }
        }
    }
}

fn check_forme_reduced_motion(docs: &[UastDocument], config: &Config, out: &mut Vec<Diagnostic>) {
    if docs.iter().any(|d| {
        d.web_edge
            .as_ref()
            .is_some_and(|w| w.forme_has_reduced_motion)
    }) {
        return;
    }
    let v = LangRuleViolation::WebEdge(WebEdgeViolation::A11y(A11yViolation::MediaFormMotion));
    for doc in docs {
        if classify_path(&doc.path, &config.domains) != DomainKind::Forme {
            continue;
        }
        let Some(we) = &doc.web_edge else { continue };
        for &span in &we.forme_spatial_motion_spans {
            if !doc_already_has_code_on_line(doc, v.code(), span) {
                out.push(rule_error(
                    "GF080",
                    format!("[{}] `Forme` stylesheet `{}` defines spatial `@keyframes`/animation without `@media (prefers-reduced-motion: reduce)`", v.code(), doc.path),
                    v.remediation(),
                    SourceLoc::new(&doc.path, span),
                ));
            }
        }
    }
}

/// Evaluates cross-file Cloudflare binding coherence, Drizzle dialect alignment, and `Forme` reduced-motion (`GF080`–`GF081`).
pub fn check_workspace_web_edge(
    docs: &[UastDocument],
    config: &Config,
) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    check_wrangler_and_bindings(docs, &mut diags);
    check_drizzle_workspace_dialects(docs, &mut diags);
    check_forme_reduced_motion(docs, config, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
