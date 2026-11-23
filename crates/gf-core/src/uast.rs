//! Unified Abstract Syntax Tree (`UAST`) hub representations.

use alloc::string::String;
use alloc::vec::Vec;

use crate::diagnostic::Span;
use crate::path::RepoPath;

/// Source language or configuration dialect mapped into the `UAST`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    /// Rust source file (`.rs`).
    Rust,
    /// Python source file (`.py`).
    Python,
    /// TypeScript / TSX source file (`.ts`, `.tsx`).
    TypeScript,
    /// Go source file (`.go`).
    Go,
    /// Container build recipe (`Dockerfile`).
    Dockerfile,
    /// TOML configuration file (`.toml`).
    Toml,
    /// Unclassified text or source file.
    Unknown,
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
    /// Root `UAST` node (typically [`UastKind::File`]).
    pub root: UastNode,
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::string::ToString;

    use super::{Language, UastDocument, UastKind, UastNode};
    use crate::{RepoPath, Span};

    #[test]
    fn builds_nested_uast_document() -> Result<(), Box<dyn core::error::Error>> {
        let path = RepoPath::new("crates/gf-core/src/lib.rs")?;
        let file_span = Span::new(1, 20)?;
        let fn_span = Span::new(5, 12)?;
        let func = UastNode::leaf(
            UastKind::Function {
                name: "compute".to_string(),
                param_count: 2,
                cognitive_complexity: 3,
            },
            fn_span,
        );
        let doc = UastDocument {
            path,
            language: Language::Rust,
            total_lines: 20,
            root: UastNode {
                kind: UastKind::File,
                span: file_span,
                children: alloc::vec![func],
            },
        };
        assert_eq!(doc.language, Language::Rust);
        assert_eq!(doc.root.children.len(), 1);
        Ok(())
    }
}
