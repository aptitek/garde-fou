//! Serializable `#![no_std]` domain findings attached to [`crate::UastDocument`] in cache `v6`.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub use crate::academic_violations::AcademicSummary;
pub use crate::compiled_violations::CompiledModuleSummary;
use crate::diagnostic::Span;
use crate::format_violations::{CssViolation, MarkdownViolation, XmlSvgViolation};

/// Category of style or asset leak detected inside `DomainKind::Code`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleLeakKind {
    /// CSS-in-JS construct (`styled.*`, `` css`...` ``, `createGlobalStyle`, `.style.`).
    CssInJs,
    /// Inline JSX/HTML `style=` attribute.
    InlineStyleAttr,
    /// Embedded `<style>` JSX/HTML element.
    StyleElement,
    /// Raw color literal (`#hex`, `rgb()`, `hsl()`, `oklch()`) in code.
    RawColorLiteral,
    /// Hardcoded CSS length unit (`px`, `rem`, `em`, `vh`, `vw`) in code.
    RawCssUnit,
    /// Embedded `data:` URI inside code (`GF064`).
    DataUri,
    /// Inline `<svg>` element inside JSX (`GF064`).
    InlineSvg,
}

/// Single style or inline-asset leak extracted from a `Code` file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleLeakFinding {
    /// Specific leak kind (`GF060` or `GF064`).
    pub kind: StyleLeakKind,
    /// Short human/agent description of the matched construct.
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Category of UI `i18n` (`GF062`) or static `a11y` (`GF066`) issue in JSX/TSX.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum UiCheckKind {
    /// Hardcoded human-facing text in JSX/attributes outside `t(...)` (`GF062`).
    UntranslatedText,
    /// `onClick` handler on non-interactive `<div/span/p>` without `role` + `tabIndex` (`GF066`).
    NonInteractiveClick,
    /// `<img>` element missing an `alt` attribute (`GF066`).
    ImgMissingAlt,
    /// `<button>` element missing an explicit `type` attribute (`GF066`).
    ButtonMissingType,
    /// Empty or icon-only `<button>` missing accessible text or `aria-label` (`GF066`).
    ButtonMissingLabel,
}

/// Single UI `i18n` or `a11y` finding extracted from JSX/TSX.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiCheckFinding {
    /// Specific UI check kind (`GF062` or `GF066`).
    pub kind: UiCheckKind,
    /// Short description of the offending JSX element or attribute.
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Category of design-token or purity violation inside a `Forme` stylesheet (`GF061`/`GF064`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FormeFindingKind {
    /// Raw color literal (`#hex`, `rgb()`, `hsl()`, `oklch()`) outside token files (`GF061`).
    RawColor,
    /// Magic `px` length literal outside token files (`GF061`).
    RawPxUnit,
    /// `!important` specificity override (`GF061`).
    ImportantOverride,
    /// Non-empty translatable `content: "..."` string inside CSS (`GF061`).
    TranslatedContentText,
    /// Embedded `url(data:...)` URI inside CSS (`GF061` / `GF064`).
    DataUri,
    /// Specification-grade CSS Color 4, token fallback, unit/z-index, or specificity violation (`GF061`).
    CssSpec(CssViolation),
}

/// Single finding extracted from a `Forme` CSS/SCSS file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormeFinding {
    /// Specific stylesheet violation category.
    pub kind: FormeFindingKind,
    /// Matched token or CSS snippet.
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Structured summary of a `Forme` stylesheet.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormeDocument {
    /// Lexical/structural findings extracted from the stylesheet.
    pub findings: Vec<FormeFinding>,
}

/// Forbidden markup or style detected inside a `Texte` translation value (`GF062`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TexteIssue {
    /// Flattened dot-separated translation key (`"nav.home"`).
    pub key: String,
    /// Description of the forbidden construct (`HTML`, `style=`, `data:`).
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Structured summary of an `i18n` dictionary file in `DomainKind::Texte`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TexteDocument {
    /// Inferred locale code (e.g. `"en"`, `"fr"`).
    pub locale: String,
    /// Deterministic set of flattened dot-separated keys defined in this locale.
    pub keys: BTreeSet<String>,
    /// Values containing forbidden HTML, inline styles, or `data:` URIs.
    pub forbidden_values: Vec<TexteIssue>,
}

/// Category of editorial purity issue in a Markdown/MDX document (`GF063` / `GF064`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContenusIssueKind {
    /// Raw HTML tag outside fenced/inline code (`GF063`).
    RawHtml,
    /// Forbidden logic, hooks, `fetch`, inline `style`, or `.css` import in MDX (`GF063`).
    ForbiddenMdxLogic,
    /// Skipped heading level such as `H1` -> `H3` (`GF063`).
    HeadingLevelJump,
    /// Multiple top-level `# H1` headings in the same document (`GF063`).
    MultipleH1,
    /// Markdown image `![](...)` with empty `alt` text (`GF063`).
    MissingImageAlt,
    /// Embedded `data:` URI inside Markdown (`GF064`).
    DataUri,
    /// Specification-grade `CommonMark`/GFM/MDX violation (`GF063`).
    MarkdownSpec(MarkdownViolation),
}

/// Single editorial purity finding in a `.md` or `.mdx` document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContenusIssue {
    /// Specific Markdown/MDX violation kind.
    pub kind: ContenusIssueKind,
    /// Short description of the violation.
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Structured summary of a Markdown or MDX document.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContenusDocument {
    /// Purity and heading-hierarchy issues found outside fenced/inline code.
    pub issues: Vec<ContenusIssue>,
    /// GitHub-slugged heading anchors declared in this document.
    #[serde(default)]
    pub heading_slugs: BTreeSet<String>,
    /// Relative cross-document `.md`/`.mdx` links extracted from this document.
    #[serde(default)]
    pub doc_links: Vec<MarkdownLinkTarget>,
}

/// Security hazard detected inside a static `.svg` asset (`GF064`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AssetHazardKind {
    /// Embedded `<script>` element inside SVG (`GF064`).
    SvgScriptTag,
    /// Inline `on*=` event handler attribute inside SVG (`GF064`).
    SvgEventHandler,
    /// Embedded `data:` URI inside SVG (`GF064`).
    SvgEmbeddedDataUri,
    /// Specification-grade XXE/DTD, SMIL XSS, `<foreignObject>`, `<use>`, or `<style>` hazard (`GF064`).
    XmlSvgSpec(XmlSvgViolation),
}

/// Single security hazard found in an `.svg` asset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetHazard {
    /// Specific SVG hazard category.
    pub kind: AssetHazardKind,
    /// Matched tag or attribute snippet.
    pub detail: String,
    /// 1-indexed source line span.
    pub span: Span,
}

/// Metadata and security summary of a static asset file (`GF064`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetDocument {
    /// File size in bytes on disk.
    pub byte_size: u64,
    /// Normalized lowercase file extension.
    pub extension: String,
    /// Security hazards detected when `extension == "svg"`.
    pub hazards: Vec<AssetHazard>,
}

/// Serializable `Cargo.toml` workspace dependency summary for `TOML-WORKSPACE-BYPASS` (`GF080`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TomlManifestSummary {
    /// `true` when this `Cargo.toml` declares a `[workspace]` table.
    pub is_workspace_root: bool,
    /// Dependency keys declared under `[workspace.dependencies]`.
    pub workspace_deps: BTreeSet<String>,
    /// `(dep_key, span)` entries in a member `Cargo.toml` lacking `workspace = true`.
    pub hardcoded_member_deps: Vec<(String, Span)>,
}

/// Relative cross-document Markdown link extracted for workspace `#anchor` verification (`GF063`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MarkdownLinkTarget {
    /// Relative target `.md` / `.mdx` path (without `#fragment` or `?query`).
    pub target_path: String,
    /// Optional `#fragment` anchor (without leading `#`).
    pub fragment: Option<String>,
    /// 1-based line span of the Markdown link.
    pub span: Span,
}
