//! Closed algebraic sum types for specification-grade CSS (`GF061`), XML/SVG (`GF064`/`GF080`),
//! TOML (`GF080`), and Markdown/MDX (`GF063`) violations, plus cross-file manifest/anchor summaries.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Closed sum type of specification-grade CSS & Design Token violations (`GF061`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CssViolation {
    /// Raw `#hex`, `rgb/hsl/oklch/color-mix` literal, or CSS Color 4 named color (`CSS-RAW-COLOR`).
    RawColor,
    /// Raw primitive fallback inside `var(--tok, #fff)` / `var(--s, 16px)` (`CSS-FALLBACK-TOKEN-LEAK`).
    FallbackTokenLeak,
    /// Raw `px`/`pt`/`cm`/`mm`/`in` length or raw integer `z-index` (`CSS-RAW-DIMENSION-OR-ZINDEX`).
    RawDimensionOrZIndex,
    /// High-specificity `#id` selector in rule prelude (`CSS-NO-ID-SELECTOR`).
    NoIdSelector,
    /// `!important` specificity override (`CSS-NO-IMPORTANT`).
    NoImportant,
    /// Runtime `@import` outside token entrypoints (`CSS-NO-AT-IMPORT`).
    NoAtImport,
    /// Layout-thrashing `transition: all` / `transition-property: all` (`CSS-NO-TRANSITION-ALL`).
    NoTransitionAll,
}

const CSS_SPECS: [(&str, &str); 7] = [
    (
        "CSS-RAW-COLOR",
        "Define primitive colors inside `tokens/` (`forme_token_globs`) and consume `var(--color-*)` or `currentColor`/`transparent`.",
    ),
    (
        "CSS-FALLBACK-TOKEN-LEAK",
        "Use another token (`var(--a, var(--b))`) or intrinsic keyword (`currentColor`, `transparent`, `inherit`) as `var()` fallback.",
    ),
    (
        "CSS-RAW-DIMENSION-OR-ZINDEX",
        "Define spacing, sizes, and elevation scales inside `tokens/` and consume `var(--space-*)` / `var(--z-*)` (only unitless `0` and `z-index: -1|0|1|auto` are exempt).",
    ),
    (
        "CSS-NO-ID-SELECTOR",
        "Replace `#id` selectors with flat semantic `.class` or `[data-*]` selectors to keep specificity predictable.",
    ),
    (
        "CSS-NO-IMPORTANT",
        "Remove `!important` and resolve cascade ordering via `@layer` or flat class specificity.",
    ),
    (
        "CSS-NO-AT-IMPORT",
        "Avoid runtime CSS `@import` waterfalls in component stylesheets; bundle imports at build time or inside `tokens/`.",
    ),
    (
        "CSS-NO-TRANSITION-ALL",
        "Specify explicit compositor-friendly properties (`transition: opacity var(--dur), transform var(--dur)`) instead of `all`.",
    ),
];

impl CssViolation {
    /// Canonical sub-rule code (`"CSS-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        CSS_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        CSS_SPECS[self as usize].1
    }

    /// Returns `true` when permitted inside Design Token definition files (`forme_token_globs`).
    #[must_use]
    pub const fn allowed_in_token_file(self) -> bool {
        matches!(
            self,
            Self::RawColor
                | Self::FallbackTokenLeak
                | Self::RawDimensionOrZIndex
                | Self::NoImportant
                | Self::NoAtImport
        )
    }
}

impl fmt::Display for CssViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Closed sum type of XML, XXE/DTD, SVG XSS/SMIL, and XML manifest violations (`GF064` / `GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum XmlSvgViolation {
    /// `<!ENTITY`, external `SYSTEM`/`PUBLIC` DTD, or SVG internal subset (`XML-FORBID-DTD-XXE`).
    ForbidDtdXxe,
    /// `<script>`, `<foreignObject>`, `<iframe>`, `<embed>`, `<object>`, or `<handler>` (`SVG-DANGEROUS-ELEMENT`).
    SvgDangerousElement,
    /// `on*=` handler or SMIL `<set>`/`<animate*>` mutating `on*`/`href` (`SVG-EVENT-OR-SMIL-XSS`).
    SvgEventOrSmilXss,
    /// `javascript:`/`vbscript:`/`data:` `href`, external `<use>`, or unsafe `<style>` (`SVG-UNSAFE-HREF-OR-USE`).
    SvgUnsafeHrefOrUse,
    /// Floating `*`/`LATEST`/`RELEASE` version or `http://` repo in `.csproj`/`.props`/`pom.xml` (`XML-UNPINNED-DEPENDENCY`).
    UnpinnedDependency,
}

const XML_SVG_SPECS: [(&str, &str); 5] = [
    (
        "XML-FORBID-DTD-XXE",
        "Remove `<!DOCTYPE ...>` internal subsets, `<!ENTITY>`, and `SYSTEM`/`PUBLIC` external DTD declarations.",
    ),
    (
        "SVG-DANGEROUS-ELEMENT",
        "Strip executable/embedded container tags (`<script>`, `<foreignObject>`, `<iframe>`, `<embed>`, `<object>`, `<handler>`) from `.svg` assets.",
    ),
    (
        "SVG-EVENT-OR-SMIL-XSS",
        "Remove `on*` event attributes and SMIL `<set>`/`<animate*>` elements targeting `on*`, `href`, or `xlink:href`.",
    ),
    (
        "SVG-UNSAFE-HREF-OR-USE",
        "Use fragment-local `#id` references in `<use>` and remove `javascript:`, `vbscript:`, `data:`, and external `@import`/`url(...)` from SVG.",
    ),
    (
        "XML-UNPINNED-DEPENDENCY",
        "Pin exact deterministic dependency versions (no `*`, `LATEST`, or `RELEASE`) and use `https://` package repositories.",
    ),
];

impl XmlSvgViolation {
    /// Canonical sub-rule code (`"XML-..."` or `"SVG-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        XML_SVG_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        XML_SVG_SPECS[self as usize].1
    }
}

impl fmt::Display for XmlSvgViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Closed sum type of TOML 1.0/1.1 & manifest supply-chain violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TomlViolation {
    /// Out-of-order table reopening, interleaved dotted subtables, or `> 4`-key inline table (`TOML-CONFUSING-TABLE-REOPEN`).
    ConfusingTableReopen,
    /// `"*"` wildcard version, `"latest"` tool pin, or `git` dependency without immutable `rev`/`tag` (`TOML-UNPINNED-GIT-OR-WILDCARD`).
    UnpinnedGitOrWildcard,
    /// Member `Cargo.toml` hardcoding a version already managed in `[workspace.dependencies]` (`TOML-WORKSPACE-BYPASS`).
    WorkspaceBypass,
}

const TOML_SPECS: [(&str, &str); 3] = [
    (
        "TOML-CONFUSING-TABLE-REOPEN",
        "Group dotted keys and subtables contiguously under a single `[table]` header and expand inline tables with `> 4` keys into standard `[table]` sections.",
    ),
    (
        "TOML-UNPINNED-GIT-OR-WILDCARD",
        "Pin explicit semantic versions instead of `\"*\"` / `\"latest\"`, and pin every `git` dependency to an immutable `rev = \"<commit-sha>\"` (or explicit release `tag`).",
    ),
    (
        "TOML-WORKSPACE-BYPASS",
        "Inherit shared workspace dependencies via `<crate>.workspace = true` (or `{ workspace = true }`) instead of duplicating version strings in member crates.",
    ),
];

impl TomlViolation {
    /// Canonical sub-rule code (`"TOML-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        TOML_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        TOML_SPECS[self as usize].1
    }
}

impl fmt::Display for TomlViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Closed sum type of `CommonMark` / GFM / MDX editorial & link-integrity violations (`GF063`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MarkdownViolation {
    /// Raw HTML element outside code spans/fences (`MD-RAW-HTML`).
    RawHtml,
    /// Logic, hooks, exports, `.css` imports, or impure JSX props in `.mdx` (`MD-FORBIDDEN-MDX-LOGIC`).
    ForbiddenMdxLogic,
    /// Multiple `# H1` headings or skipped heading level `H1 -> H3` (`MD-HEADING-HIERARCHY`).
    HeadingHierarchy,
    /// Empty link `[]()` / `[](#)`, alt-less image `![](...)`, or reversed `(text)[url]` (`MD-EMPTY-OR-REVERSED-LINK`).
    EmptyOrReversedLink,
    /// Unresolved `[text][ref]`, missing `#anchor` slug, or broken `./doc.md#anchor` (`MD-UNRESOLVED-REF-OR-ANCHOR`).
    UnresolvedRefOrAnchor,
}

const MD_SPECS: [(&str, &str); 5] = [
    (
        "MD-RAW-HTML",
        "Replace raw HTML tags with pure CommonMark/GFM syntax (or declarative PascalCase MDX components).",
    ),
    (
        "MD-FORBIDDEN-MDX-LOGIC",
        "Keep `.mdx` strictly editorial: remove `export`, React hooks, `fetch`, `.css` imports, inline `style={{...}}`, and `dangerouslySetInnerHTML`.",
    ),
    (
        "MD-HEADING-HIERARCHY",
        "Use a single `# H1` document title and increment heading levels sequentially (`H1 -> H2 -> H3`).",
    ),
    (
        "MD-EMPTY-OR-REVERSED-LINK",
        "Provide non-empty link destinations `[label](url)`, non-empty image `![alt](src)` text, and standard `[text](url)` bracket order.",
    ),
    (
        "MD-UNRESOLVED-REF-OR-ANCHOR",
        "Define every `[id]: url` reference and ensure `#anchor` / `./doc.md#anchor` links match an existing GitHub-slugged heading.",
    ),
];

impl MarkdownViolation {
    /// Canonical sub-rule code (`"MD-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        MD_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        MD_SPECS[self as usize].1
    }
}

impl fmt::Display for MarkdownViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
