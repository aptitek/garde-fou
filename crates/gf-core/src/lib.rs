//! Pure `#![no_std]` functional core types for **Garde-fou**.
//!
//! This crate defines algebraic domain types (`Span`, `RepoPath`, `UastDocument`,
//! `Diagnostic`, `Config`, and `Validation`) with zero I/O or `std` dependencies.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod config;
pub mod diagnostic;
pub mod path;
pub mod uast;
pub mod validation;

pub use config::{Config, LimitsConfig};
pub use diagnostic::{Diagnostic, Severity, Span, SpanError};
pub use path::{RepoPath, RepoPathError};
pub use uast::{Language, UastDocument, UastKind, UastNode};
pub use validation::Validation;
