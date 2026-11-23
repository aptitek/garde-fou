//! Pure `#![no_std]` functional core types for **Garde-fou**.
//!
//! This crate defines algebraic domain types (`Span`, `RepoPath`, `UastDocument`,
//! `ProjectContext`, `DockerDocument`, `Diagnostic`, `Config`, and `Validation`)
//! with zero I/O or `std` dependencies.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod config;
pub mod context;
pub mod diagnostic;
pub mod docker;
pub mod glob;
pub mod metrics;
pub mod path;
pub mod uast;
pub mod validation;

pub use config::{Config, LimitsConfig, ProjectConfig};
pub use context::{FeatureId, FrameworkId, LanguageId, ProjectContext};
pub use diagnostic::{Diagnostic, Severity, Span, SpanError};
pub use docker::{DockerDocument, DockerInstruction, DockerInstructionKind, FromStage};
pub use glob::glob_matches;
pub use metrics::{
    CallableDecl, CallableKind, ExportDecl, ImportDecl, SyntaxErrorNode, TypeDecl, TypeKind,
};
pub use path::{RepoPath, RepoPathError};
pub use uast::{Language, UastDocument, UastKind, UastNode};
pub use validation::Validation;
