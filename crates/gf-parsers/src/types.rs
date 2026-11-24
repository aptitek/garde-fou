//! Shared leaf types and extractor trait for `gf-parsers` (keeping the module graph a strict DAG).

use core::fmt;

use gf_core::{RepoPath, SpanError, UastDocument};
use tree_sitter::Node;

/// Safely slices `source` using `node.byte_range()`, returning `""` if out of bounds.
#[must_use]
pub fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or("")
}

/// High-level ecosystem or build-tool marker detected from a repository path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMarker {
    /// Rust Cargo manifest (`Cargo.toml`).
    CargoManifest,
    /// Container build recipe (`Dockerfile`, `Containerfile`).
    Containerfile,
    /// Node.js package manifest (`package.json`).
    NodeManifest,
    /// Go module manifest (`go.mod`).
    GoMod,
}

/// Errors produced while bridging source text into a [`UastDocument`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// Source span invariants failed during node construction.
    InvalidSpan(SpanError),
    /// `Tree-sitter` grammar failed to initialize on the parser instance.
    LanguageSetup,
    /// `Tree-sitter` returned no syntax tree (unexpected cancellation or engine error).
    EngineFailure,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan(err) => write!(f, "failed to build UAST span: {err}"),
            Self::LanguageSetup => f.write_str("failed to configure Tree-sitter grammar"),
            Self::EngineFailure => f.write_str("Tree-sitter parser returned no CST"),
        }
    }
}

impl core::error::Error for ParseError {}

impl From<SpanError> for ParseError {
    fn from(err: SpanError) -> Self {
        Self::InvalidSpan(err)
    }
}

/// Contract implemented by every language-specific structural extractor.
pub trait LanguageExtractor {
    /// Parses `source` and extracts a normalized [`UastDocument`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if initializing the grammar or constructing spans fails.
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError>;
}
