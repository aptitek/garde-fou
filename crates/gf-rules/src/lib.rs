//! Pure `#![no_std]` rule evaluation engine for **Garde-fou**.
//!
//! Depends exclusively on [`gf_core`] and transforms a [`UastDocument`] plus [`Config`]
//! into deterministic [`Diagnostic`] values via applicative [`Validation`] accumulation.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod checks;

use alloc::vec::Vec;
use gf_core::{Config, Diagnostic, UastDocument, Validation};

use checks::{
    check_callables, check_file_metrics, check_root_uast, check_syntax_errors, check_types,
};

/// Evaluates all pure static rules applicatively, accumulating every violation without
/// short-circuiting on the first failure.
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
    ])
}

/// Evaluates all pure static rules against a normalized [`UastDocument`] and returns
/// diagnostics sorted deterministically by file path, source span, and rule ID.
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
