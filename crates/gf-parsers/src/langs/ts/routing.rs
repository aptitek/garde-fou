//! Exact TypeScript / TSX / JavaScript / JSX extension routing and ambient `.d.ts` detection.

use gf_core::{Language, RepoPath, is_ts_declaration_path};
use tree_sitter::Node;

/// Dialect selector mapping file extensions to `LANGUAGE_TYPESCRIPT` vs `LANGUAGE_TSX`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TsDialect {
    /// Pure TypeScript (`.ts`, `.mts`, `.cts`, `.d.ts`, `.d.mts`, `.d.cts`) -> `LANGUAGE_TYPESCRIPT`.
    TypeScript,
    /// TypeScript with JSX (`.tsx`) -> `LANGUAGE_TSX`.
    Tsx,
    /// JavaScript (`.js`, `.mjs`, `.cjs`) -> `LANGUAGE_TSX` (JSX-capable for React).
    JavaScript,
    /// JavaScript with explicit JSX extension (`.jsx`) -> `LANGUAGE_TSX`.
    Jsx,
}

impl TsDialect {
    /// Maps `(path, lang)` to its exact [`TsDialect`], if part of the TS/JS family.
    #[must_use]
    pub fn from_path_and_lang(path: &RepoPath, lang: Language) -> Option<Self> {
        if is_ts_declaration_path(path.as_str()) {
            return Some(Self::TypeScript);
        }
        match lang {
            Language::TypeScript => Some(Self::TypeScript),
            Language::Tsx => Some(Self::Tsx),
            Language::JavaScript => Some(Self::JavaScript),
            Language::Jsx => Some(Self::Jsx),
            _ => None,
        }
    }

    /// Corresponding [`Language`] stored on [`gf_core::UastDocument`].
    #[must_use]
    pub const fn language(self) -> Language {
        match self {
            Self::TypeScript => Language::TypeScript,
            Self::Tsx => Language::Tsx,
            Self::JavaScript => Language::JavaScript,
            Self::Jsx => Language::Jsx,
        }
    }

    /// Embedded `tree-sitter-typescript` grammar (`LANGUAGE_TYPESCRIPT` only for non-JSX `.ts`/`.mts`/`.cts`).
    #[must_use]
    pub fn ts_grammar(self) -> tree_sitter::Language {
        match self {
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx | Self::Jsx | Self::JavaScript => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }

    /// Returns `true` when JSX elements can appear in this dialect.
    #[must_use]
    pub const fn supports_jsx(self) -> bool {
        matches!(self, Self::Tsx | Self::Jsx | Self::JavaScript)
    }
}

/// Returns `true` if `node` sits inside an `ambient_declaration` (`declare ...`) or a `.d.ts` file.
#[must_use]
pub fn is_in_ambient_context(mut node: Node<'_>, is_dts_file: bool) -> bool {
    if is_dts_file {
        return true;
    }
    while let Some(cur) = node.parent() {
        if cur.kind() == "ambient_declaration" {
            return true;
        }
        node = cur;
    }
    false
}
