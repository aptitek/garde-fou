//! Pure algebraic AST data types for raw and anchor-resolved YAML 1.2 documents.

use std::collections::BTreeMap;

use gf_core::{Span, SyntaxErrorNode};
use saphyr_parser::ScalarStyle;

/// Normalized scalar presentation style from YAML 1.2 lexical events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum YamlStyle {
    /// Unquoted plain scalar (`true`, `0755`, `22:22`, `foo`).
    Plain,
    /// Single-quoted scalar (`'0755'`).
    SingleQuoted,
    /// Double-quoted scalar (`"8080:80"`).
    DoubleQuoted,
    /// Literal block scalar (`|`, `|-`, `|+`).
    Literal,
    /// Folded block scalar (`>`, `>-`, `>+`).
    Folded,
}

impl YamlStyle {
    pub(super) const fn from_saphyr(style: ScalarStyle) -> Self {
        match style {
            ScalarStyle::Plain => Self::Plain,
            ScalarStyle::SingleQuoted => Self::SingleQuoted,
            ScalarStyle::DoubleQuoted => Self::DoubleQuoted,
            ScalarStyle::Literal => Self::Literal,
            ScalarStyle::Folded => Self::Folded,
        }
    }

    /// Returns `true` for `|` and `>` block scalars whose indicator sits on `span.start_line()`.
    #[must_use]
    pub const fn is_block(self) -> bool {
        matches!(self, Self::Literal | Self::Folded)
    }
}

/// Scalar value paired with its YAML lexical style, tag presence, and 1-based [`Span`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YamlScalar {
    /// Decoded UTF-8 scalar text.
    pub value: String,
    /// Lexical quoting/block style.
    pub style: YamlStyle,
    /// Whether an explicit YAML tag (`!!str`, `!custom`) was attached.
    pub has_tag: bool,
    /// 1-based source span in the YAML document.
    pub span: Span,
}

impl YamlScalar {
    /// Maps a 1-based line inside an embedded script back to its exact 1-based YAML line.
    #[must_use]
    pub fn remap_embedded_line(&self, script_line_1based: u32) -> u32 {
        let raw = if self.style.is_block() {
            self.span.start_line().saturating_add(script_line_1based)
        } else {
            self.span
                .start_line()
                .saturating_add(script_line_1based.saturating_sub(1))
        };
        raw.min(self.span.end_line()).max(self.span.start_line())
    }
}

/// Raw unexpanded YAML AST node preserving anchors and aliases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum YamlNode {
    /// Scalar leaf node.
    Scalar(YamlScalar),
    /// Sequence (`[...]`, `- item`).
    Sequence {
        /// Ordered child nodes.
        items: Vec<Self>,
        /// 1-based span of the sequence.
        span: Span,
    },
    /// Mapping (`{ k: v }`, `k: v`).
    Mapping {
        /// Raw `(key, value)` pairs in lexical order.
        entries: Vec<(Self, Self)>,
        /// 1-based span of the mapping.
        span: Span,
    },
    /// `*alias` reference to a previously defined `anchor_id`.
    Alias {
        /// Non-zero `saphyr-parser` anchor identifier.
        target_id: usize,
        /// 1-based span of the `*alias` token.
        span: Span,
    },
}

impl YamlNode {
    /// Returns the 1-based [`Span`] of any YAML node.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Scalar(s) => s.span,
            Self::Sequence { span, .. } | Self::Mapping { span, .. } | Self::Alias { span, .. } => {
                *span
            }
        }
    }
}

/// Parsed single YAML stream document (`---` .. `...`) with its anchor table.
#[derive(Clone, Debug, Default)]
pub struct RawYamlStreamDoc {
    /// Root node of the YAML document.
    pub root: Option<YamlNode>,
    /// Anchor definitions (`anchor_id -> YamlNode`) scoped to this YAML document.
    pub anchors: BTreeMap<usize, YamlNode>,
    /// Line ranges `(body_start_line, body_end_line)` occupied by block scalar bodies.
    pub block_body_ranges: Vec<(u32, u32)>,
}

/// Output of `saphyr-parser` over a multi-document YAML stream.
#[derive(Clone, Debug, Default)]
pub struct ParsedYamlStream {
    /// Parsed YAML documents in stream order.
    pub docs: Vec<RawYamlStreamDoc>,
    /// Lexical or grammatical syntax errors (`GF000`).
    pub syntax_errors: Vec<SyntaxErrorNode>,
}

/// Expanded YAML AST node with `*alias` and `<<` merge keys resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedYaml {
    /// Scalar leaf node.
    Scalar(YamlScalar),
    /// Sequence of resolved items.
    Sequence {
        /// Ordered items.
        items: Vec<Self>,
        /// 1-based span.
        span: Span,
    },
    /// Mapping of unique scalar keys to resolved values.
    Mapping {
        /// Deduplicated `(key, value)` pairs (explicit local keys first, then inherited `<<` keys).
        entries: Vec<(YamlScalar, Self)>,
        /// 1-based span.
        span: Span,
    },
}

impl ResolvedYaml {
    /// Returns the 1-based [`Span`] of this node.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Scalar(s) => s.span,
            Self::Sequence { span, .. } | Self::Mapping { span, .. } => *span,
        }
    }

    /// Looks up a mapping entry by exact key name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        let Self::Mapping { entries, .. } = self else {
            return None;
        };
        entries.iter().find(|(k, _)| k.value == key).map(|(_, v)| v)
    }

    /// Looks up both key scalar and value by exact key name.
    #[must_use]
    pub fn get_entry(&self, key: &str) -> Option<(&YamlScalar, &Self)> {
        let Self::Mapping { entries, .. } = self else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| k.value == key)
            .map(|(k, v)| (k, v))
    }

    /// Returns `Some(&YamlScalar)` if this node is a scalar.
    #[must_use]
    pub const fn as_scalar(&self) -> Option<&YamlScalar> {
        if let Self::Scalar(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

/// Returns `true` for Docker Compose `x-*` / `X-*` extension keys.
#[must_use]
pub const fn is_extension_key(name: &str) -> bool {
    matches!(name.as_bytes(), [b'x' | b'X', b'-', ..])
}
