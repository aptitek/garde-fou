//! Pure `#![no_std]` local, workspace-wide, 6-domain, security, and coverage rule engine.
//!
//! Depends exclusively on [`gf_core`] and transforms [`UastDocument`] values,
//! [`ProjectContext`], [`LcovSummary`], and [`Config`] into deterministic [`Diagnostic`]s.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod asset_and_script_checks;
mod checks;
mod docker_checks;
mod docker_instr_checks;
mod domain_checks;
mod i18n_checks;
mod security_checks;
mod workspace_checks;

use alloc::vec::Vec;
use gf_core::{Config, Diagnostic, ProjectContext, UastDocument, Validation};

use asset_and_script_checks::check_workspace_assets_and_scripts;
use checks::{
    check_callables, check_file_metrics, check_root_uast, check_syntax_errors, check_types,
};
use docker_checks::check_docker_feature;
use domain_checks::check_single_file_domains;
use i18n_checks::check_workspace_i18n;
use security_checks::check_native_secrets;
pub use security_checks::evaluate_lcov_coverage;
use workspace_checks::{check_dead_exports, check_type2_clones};

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
/// and workspace 6-domain rules (`GF062`, `GF064`, `GF065`).
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
