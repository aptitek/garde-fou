//! Unified Abstract Syntax Tree (`UAST`) hub representations.

use alloc::string::String;
use alloc::vec::Vec;

use crate::diagnostic::{Span, SpanError};
use crate::metrics::{CallableDecl, ExportDecl, ImportDecl, SyntaxErrorNode, TypeDecl};
use crate::path::RepoPath;

/// Source language or configuration dialect mapped into the `UAST`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    /// Rust source file (`.rs`).
    Rust,
    /// Python source file (`.py`).
    Python,
    /// TypeScript source file (`.ts`).
    TypeScript,
    /// TypeScript JSX source file (`.tsx`).
    Tsx,
    /// JavaScript source file (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// JavaScript JSX source file (`.jsx`).
    Jsx,
    /// Go source file (`.go`).
    Go,
    /// Container build recipe (`Dockerfile`).
    Dockerfile,
    /// TOML configuration file (`.toml`).
    Toml,
    /// Unclassified text or source file.
    Unknown,
}

impl Language {
    /// Returns `true` if `gf-parsers` provides a full `Tree-sitter` AST extractor.
    #[must_use]
    pub const fn is_code_analyzed(self) -> bool {
        matches!(
            self,
            Self::Rust | Self::Python | Self::TypeScript | Self::Tsx | Self::JavaScript | Self::Jsx
        )
    }
}

/// Language-agnostic semantic node categories inside a [`UastDocument`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UastKind {
    /// Root translation unit of a file.
    File,
    /// Function or method definition with normalized SRP metrics.
    Function {
        /// Identifier of the function.
        name: String,
        /// Number of declared parameters.
        param_count: u32,
        /// Computed cognitive complexity score.
        cognitive_complexity: u32,
    },
    /// Explicit call or construct flagged as potentially forbidden (`unwrap`, `panic!`, `unsafe`).
    ForbiddenCall {
        /// Matched symbol or keyword.
        symbol: String,
    },
}

/// Single node inside the Unified Abstract Syntax Tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UastNode {
    /// Semantic kind of this node.
    pub kind: UastKind,
    /// Source line span covered by this node.
    pub span: Span,
    /// Child nodes nested under this construct.
    pub children: Vec<Self>,
}

impl UastNode {
    /// Constructs a leaf `UAST` node with no children.
    #[must_use]
    pub const fn leaf(kind: UastKind, span: Span) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }
}

/// Normalized `UAST` document produced by `gf-parsers` and analyzed by `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UastDocument {
    /// Repository-relative file path.
    pub path: RepoPath,
    /// Detected language of the file.
    pub language: Language,
    /// Total number of lines in the raw source text.
    pub total_lines: u32,
    /// Total number of non-blank lines in the source text.
    pub effective_lines: u32,
    /// Syntax error or missing-token nodes detected in the CST.
    pub syntax_errors: Vec<SyntaxErrorNode>,
    /// Module import declarations extracted from the file.
    pub imports: Vec<ImportDecl>,
    /// Public symbol export declarations extracted from the file.
    pub exports: Vec<ExportDecl>,
    /// Composite types and `impl` blocks extracted from the file.
    pub types: Vec<TypeDecl>,
    /// Functions, methods, and named closures extracted from the file.
    pub callables: Vec<CallableDecl>,
    /// Root `UAST` node (typically [`UastKind::File`]).
    pub root: UastNode,
}

impl UastDocument {
    /// Constructs an empty [`UastDocument`] with initialized line metrics and file span.
    ///
    /// # Errors
    ///
    /// Returns [`SpanError`] if constructing the root span fails.
    pub fn new(
        path: RepoPath,
        language: Language,
        total_lines: u32,
        effective_lines: u32,
    ) -> Result<Self, SpanError> {
        let root_span = Span::new(1, total_lines.max(1))?;
        Ok(Self {
            path,
            language,
            total_lines,
            effective_lines,
            syntax_errors: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            types: Vec::new(),
            callables: Vec::new(),
            root: UastNode::leaf(UastKind::File, root_span),
        })
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use super::{Language, UastDocument};
    use crate::RepoPath;

    #[test]
    fn constructs_document_and_identifies_analyzed_languages()
    -> Result<(), Box<dyn core::error::Error>> {
        let doc = UastDocument::new(RepoPath::new("src/lib.rs")?, Language::Rust, 42, 35)?;
        assert!(doc.language.is_code_analyzed());
        assert!(Language::Tsx.is_code_analyzed());
        assert!(Language::Python.is_code_analyzed());
        assert!(!Language::Toml.is_code_analyzed());
        assert_eq!(doc.total_lines, 42);
        assert_eq!(doc.effective_lines, 35);
        Ok(())
    }
}
