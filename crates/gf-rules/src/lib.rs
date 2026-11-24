//! Pure `#![no_std]` local, workspace-wide, 6-domain, security, and coverage rule engine.
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
mod docker_checks;
mod docker_instr_checks;
mod domain_checks;
mod i18n_checks;
mod import_resolver;
mod kiss_yagni_checks;
mod security_checks;
mod tarjan;
mod tdd_checks;
mod tool_checks;
mod workspace_checks;

use alloc::vec::Vec;
use gf_core::{Config, Diagnostic, ProjectContext, UastDocument, Validation};

use asset_and_script_checks::check_workspace_assets_and_scripts;
use boundary_checks::check_architecture_boundaries;
use checks::{
    check_callables, check_file_metrics, check_root_uast, check_syntax_errors, check_types,
};
use docker_checks::check_docker_feature;
use domain_checks::check_single_file_domains;
use i18n_checks::check_workspace_i18n;
use kiss_yagni_checks::{check_local_kiss_yagni_and_bypass, check_single_impl_contracts};
use security_checks::check_native_secrets;
use tarjan::check_dependency_cycles;
use tdd_checks::{check_local_test_assertions, check_workspace_tdd};
use tool_checks::check_local_tool_and_lang_rules;
use workspace_checks::{check_dead_exports, check_type2_clones};
pub use {
    security_checks::evaluate_lcov_coverage,
    tarjan::find_scc_cycles,
    tdd_checks::{check_agentic_governance, check_lock_integrity},
    tool_checks::convert_external_tool_findings,
};

/// Evaluates all pure single-file rules applicatively without short-circuiting.
#[must_use]
pub fn evaluate_document_validated(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    Validation::<(), Diagnostic>::collect_all([
        check_syntax_errors(doc),
        check_file_metrics(doc, config),
        check_callables(doc, config),
        check_types(doc, config),
        check_root_uast(doc, config),
        check_native_secrets(doc),
        check_single_file_domains(doc, config),
        check_local_kiss_yagni_and_bypass(doc, config),
        check_local_test_assertions(doc, config),
        check_local_tool_and_lang_rules(doc, config),
    ])
}

/// Evaluates all pure single-file rules and returns deterministically sorted diagnostics.
#[must_use]
pub fn evaluate_document(doc: &UastDocument, config: &Config) -> Vec<Diagnostic> {
    match evaluate_document_validated(doc, config) {
        Validation::Valid(()) => Vec::new(),
        Validation::Invalid(mut diagnostics) => {
            diagnostics.sort();
            diagnostics
        }
    }
}

/// Evaluates cross-file DRY clones (`GF020`), dead exports (`GF021`), `docker` (`GF030`–`GF034`),
/// 6-domain (`GF062`–`GF065`), DAG/boundaries (`GF071`–`GF072`), YAGNI (`GF075`), and TDD (`GF076`).
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
    ])
}

/// Convenience wrapper returning sorted workspace-level diagnostics.
#[must_use]
pub fn evaluate_workspace_sorted(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Vec<Diagnostic> {
    match evaluate_workspace(docs, ctx, config) {
        Validation::Valid(()) => Vec::new(),
        Validation::Invalid(mut diagnostics) => {
            diagnostics.sort();
            diagnostics
        }
    }
}
