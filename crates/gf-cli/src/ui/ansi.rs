//! Zero-dependency ANSI CSI/OSC escape stripping, Unicode cell-width calculation,
//! and visible-width truncation.

fn skip_csi(bytes: &[u8], mut idx: usize) -> usize {
    while let Some(&b) = bytes.get(idx) {
        idx += 1;
        if (0x40..=0x7E).contains(&b) {
            break;
        }
    }
    idx
}

fn skip_osc(bytes: &[u8], mut idx: usize) -> usize {
    while let Some(&b) = bytes.get(idx) {
        if b == 0x07 {
            return idx + 1;
        }
        if b == 0x1B && bytes.get(idx + 1) == Some(&b'\\') {
            return idx + 2;
        }
        idx += 1;
    }
    idx
}

fn skip_ansi_at(bytes: &[u8], idx: usize) -> Option<usize> {
    if bytes.get(idx) != Some(&0x1B) {
        return None;
    }
    match bytes.get(idx + 1).copied() {
        Some(b'[') => Some(skip_csi(bytes, idx + 2)),
        Some(b']') => Some(skip_osc(bytes, idx + 2)),
        Some(b) if (0x40..=0x5F).contains(&b) => Some(idx + 2),
        _ => Some(idx + 1),
    }
}

/// Returns the terminal column width (`0`, `1`, or `2`) of a single Unicode scalar value.
#[must_use]
pub const fn char_display_width(ch: char) -> usize {
    let u = ch as u32;
    if u <= 0x1F || (u >= 0x7F && u <= 0x9F) || is_zero_width_mark(u) {
        0
    } else if is_wide_codepoint(u) {
        2
    } else {
        1
    }
}

const fn is_zero_width_mark(u: u32) -> bool {
    matches!(
        u,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200B..=0x200F
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0xFEFF
    )
}

const fn is_wide_codepoint(u: u32) -> bool {
    matches!(
        u,
        0x1100..=0x115F
            | 0x231A..=0x231B
            | 0x26A1
            | 0x2E80..=0x303E
            | 0x3040..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF01..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x3FFFD
    )
}

fn for_each_visible_char(s: &str, mut on_char: impl FnMut(char)) {
    let bytes = s.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if let Some(next_idx) = skip_ansi_at(bytes, idx) {
            idx = next_idx;
        } else if let Some(ch) = s.get(idx..).and_then(|rest| rest.chars().next()) {
            on_char(ch);
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
}

/// Strips all ANSI CSI (`\x1b[...m`) and OSC (`\x1b]...`) escape sequences from `s`.
#[must_use]
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for_each_visible_char(s, |ch| out.push(ch));
    out
}

/// Computes the visible column width of `s`, ignoring all ANSI CSI and OSC escape sequences.
#[must_use]
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    for_each_visible_char(s, |ch| width += char_display_width(ch));
    width
}

/// Truncates `s` to at most `max_cols` visible columns, preserving inline ANSI codes
/// and appending `…` (plus `\x1b[0m` reset when styled) if truncated.
#[must_use]
pub fn truncate_visible(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    if visible_width(s) <= max_cols {
        return String::from(s);
    }
    let (mut out, saw_ansi) = take_visible_prefix(s, max_cols.saturating_sub(1));
    out.push('…');
    if saw_ansi {
        out.push_str("\x1b[0m");
    }
    out
}

fn take_visible_prefix(s: &str, budget: usize) -> (String, bool) {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut cols = 0;
    let mut idx = 0;
    let mut saw_ansi = false;
    while idx < bytes.len() {
        if let Some(next_idx) = skip_ansi_at(bytes, idx) {
            if let Some(seq) = s.get(idx..next_idx) {
                out.push_str(seq);
                saw_ansi = true;
            }
            idx = next_idx;
            continue;
        }
        let Some(ch) = s.get(idx..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        let w = char_display_width(ch);
        if cols + w > budget {
            break;
        }
        out.push(ch);
        cols += w;
        idx += ch.len_utf8();
    }
    (out, saw_ansi)
}
