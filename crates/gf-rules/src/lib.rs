//! Pure `#![no_std]` local, workspace-wide, 6-domain, security, governance, and coverage rule engine.
//!
//! Depends exclusively on [`gf_core`] and transforms [`UastDocument`] values,
//! [`ProjectContext`], [`LcovSummary`], and [`Config`] into deterministic [`Diagnostic`]s.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod asset_and_script_checks;
mod boundary_checks;
mod checks;
mod clone_checks;
mod docker_checks;
mod docker_instr_checks;
mod domain_checks;
mod format_workspace_checks;
mod governance_checks;
mod i18n_checks;
mod import_resolver;
mod kiss_yagni_checks;
mod mutation_checks;
mod rust_import_resolver;
mod security_checks;
mod tarjan;
mod tdd_checks;
mod tdd_parity;
mod tool_checks;
mod ts_import_resolver;
mod type_and_uast_checks;
mod workspace_checks;

use alloc::vec::Vec;
use gf_core::{Config, Diagnostic, ProjectContext, UastDocument, Validation};

use {
    asset_and_script_checks::check_workspace_assets_and_scripts,
    boundary_checks::check_architecture_boundaries,
    checks::{check_callables, check_file_metrics, check_syntax_errors},
    clone_checks::check_type2_clones,
    docker_checks::check_docker_feature,
    domain_checks::check_single_file_domains,
    format_workspace_checks::check_workspace_formats,
    i18n_checks::check_workspace_i18n,
    kiss_yagni_checks::{check_local_kiss_yagni_and_bypass, check_single_impl_contracts},
    security_checks::check_native_secrets,
    tarjan::check_dependency_cycles,
    tdd_checks::check_local_test_assertions,
    tdd_parity::check_workspace_tdd,
    tool_checks::{check_local_tool_and_lang_rules, check_workspace_tsconfigs},
    type_and_uast_checks::{check_root_uast, check_types},
    workspace_checks::check_dead_exports,
};
pub use {
    checks::SourceLoc,
    governance_checks::{
        check_agentic_governance, check_architecture_purity, check_lock_integrity,
    },
    mutation_checks::evaluate_mutation_summary,
    security_checks::{evaluate_crap_scores, evaluate_lcov_coverage},
    tarjan::find_scc_cycles,
    tool_checks::convert_external_tool_findings,
};

fn check_local_srp_and_uast(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    Validation::<(), Diagnostic>::collect_all([
        check_syntax_errors(doc),
        check_file_metrics(doc, config),
        check_callables(doc, config),
        check_types(doc, config),
        check_root_uast(doc, config),
    ])
}

fn check_local_domains_and_fortress(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    Validation::<(), Diagnostic>::collect_all([
        check_native_secrets(doc),
        check_single_file_domains(doc, config),
        check_local_kiss_yagni_and_bypass(doc, config),
        check_local_test_assertions(doc, config),
        check_local_tool_and_lang_rules(doc, config),
    ])
}

fn into_sorted_diagnostics(validation: Validation<(), Diagnostic>) -> Vec<Diagnostic> {
    match validation {
        Validation::Valid(()) => Vec::new(),
        Validation::Invalid(mut diagnostics) => {
            diagnostics.sort();
            diagnostics
        }
    }
}

/// Evaluates all pure single-file rules applicatively without short-circuiting.
#[must_use]
pub fn evaluate_document_validated(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    Validation::<(), Diagnostic>::collect_all([
        check_local_srp_and_uast(doc, config),
        check_local_domains_and_fortress(doc, config),
    ])
}

/// Evaluates all pure single-file rules and returns deterministically sorted diagnostics.
#[must_use]
pub fn evaluate_document(doc: &UastDocument, config: &Config) -> Vec<Diagnostic> {
    into_sorted_diagnostics(evaluate_document_validated(doc, config))
}

/// Evaluates all cross-file workspace rules applicatively (`GF020`–`GF021`, `GF030`–`GF034`,
/// `GF062`–`GF065`, `GF071`–`GF072`, `GF075`–`GF076`, `GF078`).
#[must_use]
pub fn evaluate_workspace(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    Validation::<(), Diagnostic>::collect_all([
        check_type2_clones(docs, config),
        check_dead_exports(docs, ctx, config),
        check_docker_feature(docs, ctx),
        check_workspace_i18n(docs, ctx, config),
        check_workspace_assets_and_scripts(docs, config),
        check_dependency_cycles(docs),
        check_architecture_boundaries(docs, ctx, config),
        check_single_impl_contracts(docs, ctx, config),
        check_workspace_tdd(docs, ctx, config),
        check_workspace_tsconfigs(docs),
        check_workspace_formats(docs),
        check_architecture_purity(docs, config),
    ])
}

/// Convenience wrapper returning sorted workspace-level diagnostics.
#[must_use]
pub fn evaluate_workspace_sorted(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Vec<Diagnostic> {
    into_sorted_diagnostics(evaluate_workspace(docs, ctx, config))
}
