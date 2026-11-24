//! Pure `#![no_std]` functional core types for **Garde-fou**.
//!
//! This crate defines algebraic domain types (`Span`, `RepoPath`, `UastDocument`,
//! `ProjectContext`, `DomainKind`, `DockerDocument`, `LcovSummary`, `Diagnostic`,
//! `Config`, and `Validation`) with zero I/O or `std` dependencies.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod boundaries;
pub mod clean_code;
pub mod commit_msg;
pub mod config;
pub mod context;
pub mod diagnostic;
pub mod docker;
pub mod domain_summaries;
pub mod domains;
pub mod gitignore;
pub mod glob;
pub mod lcov;
pub mod metrics;
pub mod path;
pub mod policies;
pub mod uast;
pub mod validation;

pub use config::{
    Config, CoverageConfig, LimitsConfig, ProjectConfig, ScannerPolicy, ScannersConfig,
};
pub use context::{FeatureId, FrameworkId, LanguageId, ProjectContext};
pub use diagnostic::{Diagnostic, Severity, Span, SpanError};
pub use docker::{DockerDocument, DockerInstruction, DockerInstructionKind, FromStage};
pub use domain_summaries::{
    AssetDocument, AssetHazard, AssetHazardKind, ContenusDocument, ContenusIssue,
    ContenusIssueKind, FormeDocument, FormeFinding, FormeFindingKind, StyleLeakFinding,
    StyleLeakKind, TexteDocument, TexteIssue, UiCheckFinding, UiCheckKind,
};
pub use domains::{
    DEFAULT_MAX_ASSET_BYTES, DomainKind, DomainLayoutConfig, classify_path, is_asset_extension,
    is_binary_asset_extension, is_forme_token_path, is_root_governance_doc,
};
pub use gitignore::{
    MANAGED_END, MANAGED_START, compose_dockerignore, compose_gitignore, merge_managed_block,
};
pub use glob::glob_matches;
pub use lcov::{FileCoverage, LcovSummary, parse_lcov};
pub use metrics::{
    CallableDecl, CallableKind, ExportDecl, ImportDecl, SecretFinding, SyntaxErrorNode, TypeDecl,
    TypeKind,
};
pub use path::{RepoPath, RepoPathError};
pub use uast::{Language, UastDocument, UastKind, UastNode};
pub use validation::Validation;
pub use {
    boundaries::{BoundaryRule, IntegrityConfig, LockStatus, default_atomic_design_boundaries},
    clean_code::{
        BypassFinding, CleanCodeConfig, ContractDecl, ContractImpl, KissYagniFinding,
        KissYagniKind, TddConfig, TddParityGates, TestCaseSummary, YagniGates,
    },
    commit_msg::{CommitConfig, validate_commit_message},
};
