//! Context detectors and source-to-[`UastDocument`] bridges for **Garde-fou**.
//!
//! This crate performs pure lexical/syntactic detection on in-memory strings and
//! [`RepoPath`] metadata without touching the filesystem or environment.

use core::fmt;

use gf_core::{Language, RepoPath, Span, SpanError, UastDocument, UastKind, UastNode};

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
    /// Source span invariants failed during root node construction.
    InvalidSpan(SpanError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan(err) => write!(f, "failed to build UAST span: {err}"),
        }
    }
}

impl core::error::Error for ParseError {}

impl From<SpanError> for ParseError {
    fn from(err: SpanError) -> Self {
        Self::InvalidSpan(err)
    }
}

/// Infers the [`Language`] of a repository file from its name or extension.
#[must_use]
pub fn detect_language(path: &RepoPath) -> Language {
    if path.file_name().eq_ignore_ascii_case("dockerfile") {
        return Language::Dockerfile;
    }
    match path.extension() {
        Some("rs") => Language::Rust,
        Some("py") => Language::Python,
        Some("ts" | "tsx") => Language::TypeScript,
        Some("go") => Language::Go,
        Some("dockerfile") => Language::Dockerfile,
        Some("toml") => Language::Toml,
        _ => Language::Unknown,
    }
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

/// Builds a minimal normalized [`UastDocument`] from raw UTF-8 source text.
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document span fails.
pub fn parse_source_skeleton(path: RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let language = detect_language(&path);
    let counted_lines = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    let end_line = counted_lines.max(1);
    let root_span = Span::new(1, end_line)?;

    Ok(UastDocument {
        path,
        language,
        total_lines: counted_lines,
        root: UastNode::leaf(UastKind::File, root_span),
    })
}

#[cfg(test)]
mod tests {
    use gf_core::{Language, RepoPath, SpanError};

    use super::{
        ParseError, ProjectMarker, detect_language, detect_project_marker, parse_source_skeleton,
    };

    #[test]
    fn detects_languages_and_project_markers() -> Result<(), Box<dyn core::error::Error>> {
        assert_eq!(
            detect_language(&RepoPath::new("src/lib.rs")?),
            Language::Rust
        );
        assert_eq!(detect_language(&RepoPath::new("app.py")?), Language::Python);
        assert_eq!(
            detect_language(&RepoPath::new("ui.tsx")?),
            Language::TypeScript
        );
        assert_eq!(detect_language(&RepoPath::new("main.go")?), Language::Go);
        assert_eq!(
            detect_language(&RepoPath::new("Dockerfile")?),
            Language::Dockerfile
        );
        assert_eq!(
            detect_language(&RepoPath::new("ci.dockerfile")?),
            Language::Dockerfile
        );
        assert_eq!(
            detect_language(&RepoPath::new("garde-fou.toml")?),
            Language::Toml
        );
        assert_eq!(
            detect_language(&RepoPath::new("README.md")?),
            Language::Unknown
        );

        assert_eq!(
            detect_project_marker(&RepoPath::new("Cargo.toml")?),
            Some(ProjectMarker::CargoManifest)
        );
        assert_eq!(
            detect_project_marker(&RepoPath::new("Dockerfile")?),
            Some(ProjectMarker::Containerfile)
        );
        assert_eq!(
            detect_project_marker(&RepoPath::new("package.json")?),
            Some(ProjectMarker::NodeManifest)
        );
        assert_eq!(
            detect_project_marker(&RepoPath::new("go.mod")?),
            Some(ProjectMarker::GoMod)
        );
        assert_eq!(detect_project_marker(&RepoPath::new("src/lib.rs")?), None);
        Ok(())
    }

    #[test]
    fn parses_empty_and_multiline_skeletons() -> Result<(), Box<dyn core::error::Error>> {
        let empty = parse_source_skeleton(RepoPath::new("empty.rs")?, "")?;
        assert_eq!(empty.total_lines, 0);
        assert_eq!(empty.root.span.line_count(), 1);

        let multi = parse_source_skeleton(RepoPath::new("two.rs")?, "fn a() {}\nfn b() {}\n")?;
        assert_eq!(multi.total_lines, 2);
        assert_eq!(multi.root.span.end_line(), 2);

        let err = ParseError::from(SpanError::ZeroLine);
        assert_ne!(err.to_string(), "");
        Ok(())
    }
}
