//! Algebraic AST node definitions for W3C CSS rules, declarations, and component values.

use super::tokenizer::{CssTokKind, CssToken};

/// Parsed CSS component value preserving nested function calls (`var(...)`, `calc(...)`, `url(...)`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssValue {
    /// Atomic lexical token.
    Token(CssToken),
    /// Function call `<name>(<args>)`.
    Function {
        /// Function identifier (`"var"`, `"calc"`, `"url"`, `"color-mix"`).
        name: String,
        /// 1-based starting line of the function token.
        line: u32,
        /// Component values inside `(...)`.
        args: Vec<Self>,
    },
}

/// Parsed `property: value [!important];` CSS declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssDeclaration {
    /// Property name (`"color"`, `"--bg"`, `"transition"`, `"z-index"`).
    pub property: String,
    /// 1-based starting line of the declaration.
    pub line: u32,
    /// Component values on the right-hand side (excluding trailing `!important`).
    pub values: Vec<CssValue>,
    /// `true` when terminated by `!important`.
    pub important: bool,
}

/// Top-level or nested CSS rule node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssRule {
    /// `@import`, `@media`, `@layer`, `@keyframes`, or `@property` rule.
    AtRule {
        /// Lowercase at-rule name without `@`.
        name: String,
        /// 1-based line of `@<name>`.
        line: u32,
        /// Prelude component values before `;` or `{`.
        prelude: Vec<CssValue>,
        /// Declarations directly inside `{ ... }` (e.g. `@property`, `@font-face`).
        declarations: Vec<CssDeclaration>,
        /// Nested rules inside `{ ... }` (e.g. `@media`, `@layer`, `@keyframes`).
        nested_rules: Vec<Self>,
    },
    /// Qualified style rule `<selector-prelude> { <declarations / nested-rules> }`.
    QualifiedRule {
        /// Selector prelude component values before `{`.
        prelude: Vec<CssValue>,
        /// Style declarations inside `{ ... }`.
        declarations: Vec<CssDeclaration>,
        /// Nested rules (CSS Nesting / SCSS) inside `{ ... }`.
        nested_rules: Vec<Self>,
    },
}

impl CssRule {
    /// Direct declarations inside this rule block.
    #[must_use]
    pub fn declarations(&self) -> &[CssDeclaration] {
        match self {
            Self::AtRule { declarations, .. } | Self::QualifiedRule { declarations, .. } => {
                declarations
            }
        }
    }

    /// Direct nested rules inside this rule block.
    #[must_use]
    pub fn nested_rules(&self) -> &[Self] {
        match self {
            Self::AtRule { nested_rules, .. } | Self::QualifiedRule { nested_rules, .. } => {
                nested_rules
            }
        }
    }
}

pub(super) fn strip_trailing_important(values: &mut Vec<CssValue>) -> bool {
    if let [
        ..,
        CssValue::Token(CssToken {
            kind: CssTokKind::Bang,
            ..
        }),
        CssValue::Token(CssToken {
            kind: CssTokKind::Ident(id),
            ..
        }),
    ] = values.as_slice()
        && id.eq_ignore_ascii_case("important")
    {
        values.pop();
        values.pop();
        return true;
    }
    false
}

pub(super) fn extract_url_payload(args: &[CssValue]) -> Option<&str> {
    args.iter().find_map(|arg| match arg {
        CssValue::Token(CssToken {
            kind: CssTokKind::RawUrl(s) | CssTokKind::Str(s),
            ..
        }) => Some(s.trim()),
        CssValue::Function { .. } | CssValue::Token(_) => None,
    })
}
