//! Pure byte-offset -> 1-based line/column helpers for `toml_edit` spans.

use gf_core::Span;

/// Computes the 1-based [`Span`] of `byte_offset` inside `source`.
#[must_use]
pub fn span_from_byte_offset(source: &str, byte_offset: usize) -> Span {
    let prefix = source.get(..byte_offset).unwrap_or(source);
    let line = u32::try_from(prefix.bytes().filter(|&b| b == b'\n').count())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    Span::new(line, line).unwrap_or(Span::FIRST)
}

/// Computes the 1-based UTF-8 column of `byte_offset` inside `source`.
#[must_use]
pub fn col_from_byte_offset(source: &str, byte_offset: usize) -> usize {
    let prefix = source.get(..byte_offset).unwrap_or(source);
    let line_start = prefix.rfind('\n').map_or(0, |idx| idx + 1);
    prefix[line_start..].chars().count().saturating_add(1)
}
