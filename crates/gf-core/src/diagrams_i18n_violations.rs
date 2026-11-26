//! Closed algebraic sum types for Mermaid v11 (`MERMAID-*`), `PlantUML` (`PLANTUML-*`),
//! and `i18n` v2 ICU/Gettext/ARB/Fluent (`I18N-*`) guardrails (`GF062`, `GF063`, `GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Mermaid v11 diagram syntax, complexity, and anti-XSS violations (`MERMAID-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MermaidViolation {
    /// Empty diagram or unrecognized Mermaid v11 header keyword.
    UnknownOrEmptyDiagram,
    /// Unclosed `subgraph`/`alt`/`opt`/`loop`/`par`/`rect`/`box` block or unbalanced node shape/quotes.
    UnclosedBlockOrShape,
    /// Unquoted lowercase `end` used as a flowchart node identifier (corrupts `subgraph` closing).
    ReservedEndNode,
    /// Diagram exceeds `[diagrams] max_nodes` distinct nodes (spaghetti diagram).
    SpaghettiComplexity,
    /// `securityLevel: 'loose'|'antiscript'`, `click ... javascript:/call`, raw `<script>`/`on*=`, or forbidden inline `#hex`.
    UnsafeSecurityOrXss,
}

impl MermaidViolation {
    /// Canonical `MERMAID-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "MERMAID-UNKNOWN-OR-EMPTY-DIAGRAM",
            "MERMAID-UNCLOSED-BLOCK-OR-SHAPE",
            "MERMAID-RESERVED-END-NODE",
            "MERMAID-SPAGHETTI-COMPLEXITY",
            "MERMAID-UNSAFE-SECURITY-OR-XSS",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 5] = [
            "Start the Mermaid diagram with an official v11 header (`flowchart`, `sequenceDiagram`, `classDiagram`, `erDiagram`, `C4Context`, etc.).",
            "Close every `subgraph`/`alt`/`opt`/`loop`/`par`/`rect`/`box` with `end` and balance node shape delimiters (`[]`, `()`, `{}`, `\"\"`).",
            "Rename or quote the lowercase `end` node (e.g. `END_NODE[\"end\"]`) so Mermaid never confuses it with a block terminator.",
            "Split large Mermaid diagrams into focused sub-diagrams within `[diagrams] max_nodes`.",
            "Keep `securityLevel: 'strict'`, remove `javascript:`/`call` click handlers and raw HTML `<script>`/`on*=` attributes.",
        ];
        REM[self as usize]
    }
}

/// `PlantUML` structural, anti-SSRF, include integrity, and anti-XSS violations (`PLANTUML-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PlantUmlViolation {
    /// Mismatched `@start*`/`@end*` delimiters or unclosed `{ ... }` / `alt`..`end` / `note`..`end note` blocks.
    UnbalancedStructure,
    /// Remote `!include http(s)://...`, `!includeurl`, `!import`, absolute path, `..` traversal, or `%getenv`/`%file_exists`/`%dirpath`/`!log`.
    SsrfOrLocalLeak,
    /// Relative local `!include ./file.iuml` pointing to a missing file in the workspace.
    BrokenInclude,
    /// `PlantUML` hyperlink using `[[javascript:...]]`, `[[vbscript:...]]`, or `[[data:...]]`.
    XssLink,
}

impl PlantUmlViolation {
    /// Canonical `PLANTUML-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "PLANTUML-UNBALANCED-STRUCTURE",
            "PLANTUML-SSRF-OR-LOCAL-LEAK",
            "PLANTUML-BROKEN-INCLUDE",
            "PLANTUML-XSS-LINK",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 4] = [
            "Match every `@start<kind>` with `@end<kind>` and close all `{ ... }`, `alt`/`opt`/`loop`, and `note` blocks.",
            "Remove remote `!include`/`!includeurl`, absolute/parent-traversal paths, and `%getenv`/`%file_exists`/`!log`; use stdlib `<C4/...>` or local repo includes.",
            "Fix or create the referenced local `.iuml`/`.puml` file for every relative `!include`.",
            "Replace `[[javascript:...]]`/`[[data:...]]` links with safe `https://` or relative documentation links.",
        ];
        REM[self as usize]
    }
}

/// Universal `i18n` v2 ICU `MessageFormat`, Gettext `.po`, ARB `.arb`, and Fluent `.ftl` violations (`I18N-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum I18nViolation {
    /// Unbalanced `{`/`}`, unknown ICU type, invalid CLDR plural category, or missing mandatory `other` branch.
    InvalidIcuSyntax,
    /// Two locales interpolate different placeholder variable sets (`{name}` vs `{username}`, `%1$s` vs `%2$s`).
    PlaceholderMismatch,
    /// Empty translation string (outside `.po` header), `#, fuzzy` Gettext entry, or XSS markup (`<script>`/`javascript:`).
    EmptyOrFuzzyOrXss,
}

impl I18nViolation {
    /// Canonical `I18N-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 3] = [
            "I18N-INVALID-ICU-SYNTAX",
            "I18N-PLACEHOLDER-MISMATCH",
            "I18N-EMPTY-OR-FUZZY-OR-XSS",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 3] = [
            "Fix ICU MessageFormat braces, use valid CLDR plural categories (`zero|one|two|few|many|other|=N`), and always include an `other { ... }` branch.",
            "Ensure every locale interpolates the exact same placeholder variable set for each translation key.",
            "Provide non-empty translations, resolve `#, fuzzy` Gettext entries, and remove raw `<script>`/`javascript:` markup.",
        ];
        REM[self as usize]
    }
}

/// Unified closed sum type across Diagrams-as-Code (`Mermaid`/`PlantUML`) and `i18n` v2 (`GF062`/`GF063`/`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DiagramI18nViolation {
    /// Mermaid v11 violation (`MERMAID-*`).
    Mermaid(MermaidViolation),
    /// `PlantUML` violation (`PLANTUML-*`).
    PlantUml(PlantUmlViolation),
    /// `i18n` v2 violation (`I18N-*`).
    I18n(I18nViolation),
}

impl DiagramI18nViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Mermaid(v) => v.code(),
            Self::PlantUml(v) => v.code(),
            Self::I18n(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Mermaid(v) => v.remediation(),
            Self::PlantUml(v) => v.remediation(),
            Self::I18n(v) => v.remediation(),
        }
    }
}

impl fmt::Display for DiagramI18nViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
