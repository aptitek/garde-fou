//! Unified Abstract Syntax Tree (`UAST`) hub representations (`v4` fortress schema).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::clean_code::{
    BypassFinding, ContractDecl, ContractImpl, KissYagniFinding, TestCaseSummary,
};
use crate::diagnostic::{Span, SpanError};
use crate::docker::DockerDocument;
use crate::domain_summaries::{
    AssetDocument, ContenusDocument, FormeDocument, StyleLeakFinding, TexteDocument, UiCheckFinding,
};
use crate::metrics::{
    CallableDecl, ExportDecl, ImportDecl, SecretFinding, SyntaxErrorNode, TypeDecl,
};
use crate::path::RepoPath;

/// Source language, stylesheet, content, or asset format mapped into the `UAST`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    /// Container build recipe (`Dockerfile`, `Containerfile`).
    Dockerfile,
    /// Stylesheet in `Forme` (`.css`, `.scss`).
    Css,
    /// Editorial or governance Markdown/MDX (`.md`, `.mdx`).
    Markdown,
    /// JSON document or `i18n` dictionary (`.json`).
    Json,
    /// TOML configuration or `i18n` dictionary (`.toml`).
    Toml,
    /// Shell / PowerShell automation script (`.sh`, `.bash`, `.ps1`).
    Shell,
    /// Scalable Vector Graphics XML asset (`.svg`).
    Svg,
    /// Non-text binary media or font asset (`.png`, `.woff2`, `.pdf`, etc.).
    BinaryAsset,
    /// Unclassified text or source file.
    Unknown,
}

impl Language {
    /// Returns `true` if `gf-parsers` provides a programming-language / Dockerfile AST extractor.
    #[must_use]
    pub const fn is_code_analyzed(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::Python
                | Self::TypeScript
                | Self::Tsx
                | Self::JavaScript
                | Self::Jsx
                | Self::Dockerfile
        )
    }

    /// Returns `true` for all file formats analyzed directly across the 6 domains
    /// (excluding generic `Json`/`Toml`, which are included selectively inside `Texte`).
    #[must_use]
    pub const fn is_always_discovered(self) -> bool {
        self.is_code_analyzed()
            || matches!(
                self,
                Self::Css | Self::Markdown | Self::Shell | Self::Svg | Self::BinaryAsset
            )
    }
}

/// Language-agnostic semantic node categories inside a [`UastDocument`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

/// Normalized `UAST` document produced by `gf-parsers`, cached in `.garde-fou-cache/v4/`,
/// and analyzed locally and globally by `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UastDocument {
    /// Repository-relative file path.
    pub path: RepoPath,
    /// Detected language or domain format of the file.
    pub language: Language,
    /// Total number of lines in the raw source text.
    pub total_lines: u32,
    /// Total number of non-blank lines in the source text.
    pub effective_lines: u32,
    /// Syntax error or missing-token nodes detected in the file.
    pub syntax_errors: Vec<SyntaxErrorNode>,
    /// Module import declarations extracted from the file.
    pub imports: Vec<ImportDecl>,
    /// Public symbol export declarations extracted from the file.
    pub exports: Vec<ExportDecl>,
    /// Composite types and `impl` blocks extracted from the file.
    pub types: Vec<TypeDecl>,
    /// Functions, methods, and named closures extracted from the file.
    pub callables: Vec<CallableDecl>,
    /// Redacted native secret findings (`GF040`) extracted at parse time.
    pub secret_findings: Vec<SecretFinding>,
    /// Deterministic set of symbol identifiers referenced inside this file.
    pub referenced_symbols: BTreeSet<String>,
    /// Style or inline-asset leaks detected in `Code` (`GF060`, `GF064`).
    pub style_leaks: Vec<StyleLeakFinding>,
    /// UI `i18n` (`GF062`) and static `a11y` (`GF066`) findings in JSX/TSX.
    pub ui_checks: Vec<UiCheckFinding>,
    /// Translation keys passed to `t("...")` / `i18n.t("...")` (`GF062`).
    pub translation_keys_used: BTreeSet<String>,
    /// Local asset paths referenced in `Code`, `Forme`, or `Contenus` (`GF064`).
    pub asset_refs: BTreeSet<String>,
    /// Script paths or filenames mentioned in string literals (`GF065` test coverage).
    pub script_refs: BTreeSet<String>,
    /// Linter-bypass directives/attributes detected in CST (`GF070`).
    pub bypass_findings: Vec<BypassFinding>,
    /// Local KISS (`GF073`) and YAGNI (`GF074`) smells.
    pub kiss_yagni_findings: Vec<KissYagniFinding>,
    /// Behavioral contracts (`trait` / `interface` / `Protocol` with methods, `GF075`).
    pub contracts: Vec<ContractDecl>,
    /// Concrete `(contract, implementor)` bindings declared in this file (`GF075`).
    pub contract_impls: BTreeSet<ContractImpl>,
    /// Unit/integration test cases with assertion counts and tautology flags (`GF076`).
    pub test_cases: Vec<TestCaseSummary>,
    /// Optional structured `Dockerfile` representation.
    pub docker: Option<DockerDocument>,
    /// Optional `Forme` stylesheet summary (`GF061`).
    pub forme: Option<FormeDocument>,
    /// Optional `Texte` (`i18n`) dictionary summary (`GF062`).
    pub texte: Option<TexteDocument>,
    /// Optional `Contenus` Markdown/MDX summary (`GF063`).
    pub contenus: Option<ContenusDocument>,
    /// Optional `Asset` metadata & SVG security summary (`GF064`).
    pub asset: Option<AssetDocument>,
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
            secret_findings: Vec::new(),
            referenced_symbols: BTreeSet::new(),
            style_leaks: Vec::new(),
            ui_checks: Vec::new(),
            translation_keys_used: BTreeSet::new(),
            asset_refs: BTreeSet::new(),
            script_refs: BTreeSet::new(),
            bypass_findings: Vec::new(),
            kiss_yagni_findings: Vec::new(),
            contracts: Vec::new(),
            contract_impls: BTreeSet::new(),
            test_cases: Vec::new(),
            docker: None,
            forme: None,
            texte: None,
            contenus: None,
            asset: None,
            root: UastNode::leaf(UastKind::File, root_span),
        })
    }
}
