//! Context detectors and safe `Tree-sitter` -> [`UastDocument`] extractors for **Garde-fou**.
//!
//! This crate performs pure syntactic and metric extraction on in-memory UTF-8 strings
//! without touching the filesystem, processes, or environment.

mod complexity;
mod cst;
pub mod langs;

use core::fmt;

use gf_core::{Language, RepoPath, SpanError, UastDocument};
use langs::{
    python::PythonExtractor,
    rust::RustExtractor,
    typescript::{TsDialect, TypeScriptExtractor},
};

/// High-level ecosystem or build-tool marker detected from a repository path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMarker {
    /// Rust Cargo manifest (`Cargo.toml`).
    CargoManifest,
    /// Container build recipe (`Dockerfile`).
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

/// Contract implemented by every language-specific `Tree-sitter` extractor.
pub trait LanguageExtractor {
    /// Parses `source` and extracts a normalized [`UastDocument`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if initializing the grammar or constructing spans fails.
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError>;
}

const EXT_LANGUAGES: &[(&str, Language)] = &[
    ("rs", Language::Rust),
    ("py", Language::Python),
    ("ts", Language::TypeScript),
    ("tsx", Language::Tsx),
    ("js", Language::JavaScript),
    ("mjs", Language::JavaScript),
    ("cjs", Language::JavaScript),
    ("jsx", Language::Jsx),
    ("go", Language::Go),
    ("dockerfile", Language::Dockerfile),
    ("toml", Language::Toml),
];

/// Infers the [`Language`] of a repository file from its name or extension.
#[must_use]
pub fn detect_language(path: &RepoPath) -> Language {
    if path.file_name().eq_ignore_ascii_case("dockerfile") {
        return Language::Dockerfile;
    }
    let Some(ext) = path.extension() else {
        return Language::Unknown;
    };
    EXT_LANGUAGES
        .iter()
        .find_map(|(candidate, lang)| (*candidate == ext).then_some(*lang))
        .unwrap_or(Language::Unknown)
}

/// Detects well-known project ecosystem manifest files from a [`RepoPath`].
#[must_use]
pub fn detect_project_marker(path: &RepoPath) -> Option<ProjectMarker> {
    match path.file_name() {
        "Cargo.toml" => Some(ProjectMarker::CargoManifest),
        "Dockerfile" => Some(ProjectMarker::Containerfile),
        "package.json" => Some(ProjectMarker::NodeManifest),
        "go.mod" => Some(ProjectMarker::GoMod),
        _ => None,
    }
}

/// Parses `source` using the dedicated `Tree-sitter` grammar for `path` when supported,
/// falling back to line-metric skeleton extraction for configuration dialects.
///
/// # Errors
///
/// Returns [`ParseError`] if `Tree-sitter` setup or span construction fails.
pub fn parse_source(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    match detect_language(path) {
        Language::Rust => RustExtractor.extract(path, source),
        Language::TypeScript => {
            TypeScriptExtractor::new(TsDialect::TypeScript).extract(path, source)
        }
        Language::JavaScript => {
            TypeScriptExtractor::new(TsDialect::JavaScript).extract(path, source)
        }
        Language::Tsx => TypeScriptExtractor::new(TsDialect::Tsx).extract(path, source),
        Language::Jsx => TypeScriptExtractor::new(TsDialect::Jsx).extract(path, source),
        Language::Python => PythonExtractor.extract(path, source),
        other => {
            let (total_lines, effective_lines) = cst::line_metrics(source);
            Ok(UastDocument::new(
                path.clone(),
                other,
                total_lines,
                effective_lines,
            )?)
        }
    }
}
