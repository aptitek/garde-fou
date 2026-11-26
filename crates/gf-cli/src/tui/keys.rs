//! Pure algebraic keyboard input model (`Key`) and VT100/CSI byte-sequence parser.

/// Normalized keyboard event consumed by pure TUI state machines (`WizardModel`, `ExplorerModel`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    /// Up arrow / previous item.
    Up,
    /// Down arrow / next item.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Jump to first item (`Home`).
    Home,
    /// Jump to last item (`End`).
    End,
    /// Confirm / submit (`Enter`).
    Enter,
    /// Cancel / blur / exit (`Escape`).
    Esc,
    /// Forward focus / toggle (`Tab`).
    Tab,
    /// Backward focus (`Shift+Tab`).
    BackTab,
    /// Delete previous character (`Backspace`).
    Backspace,
    /// Immediate interrupt (`Ctrl+C`).
    CtrlC,
    /// Select / toggle all (`Ctrl+A`).
    CtrlA,
    /// Printable Unicode scalar (e.g. `' '`, `'/'`, `'j'`, `'k'`, `'é'`).
    Char(char),
}

const fn parse_csi_tilde(param: u8) -> Option<Key> {
    match param {
        b'1' | b'7' => Some(Key::Home),
        b'4' | b'8' => Some(Key::End),
        b'5' => Some(Key::PageUp),
        b'6' => Some(Key::PageDown),
        _ => None,
    }
}

const fn parse_csi_letter(b: u8) -> Option<Key> {
    match b {
        b'A' => Some(Key::Up),
        b'B' => Some(Key::Down),
        b'C' => Some(Key::Right),
        b'D' => Some(Key::Left),
        b'H' => Some(Key::Home),
        b'F' => Some(Key::End),
        b'Z' => Some(Key::BackTab),
        _ => None,
    }
}

const fn parse_esc_sequence(bytes: &[u8]) -> (Key, usize) {
    if bytes.len() >= 3 && (bytes[1] == b'[' || bytes[1] == b'O') {
        if bytes.len() >= 4
            && bytes[1] == b'['
            && bytes[3] == b'~'
            && let Some(k) = parse_csi_tilde(bytes[2])
        {
            return (k, 4);
        }
        if let Some(k) = parse_csi_letter(bytes[2]) {
            return (k, 3);
        }
    }
    (Key::Esc, 1)
}

fn decode_utf8_prefix(bytes: &[u8]) -> Option<(char, usize)> {
    let max_len = bytes.len().min(4);
    for len in 1..=max_len {
        if let Ok(s) = core::str::from_utf8(&bytes[..len])
            && let Some(ch) = s.chars().next()
        {
            return Some((ch, ch.len_utf8()));
        }
    }
    None
}

/// Parses the leading keyboard event from raw terminal bytes, returning `(Key, bytes_consumed)`.
///
/// Returns `None` when `bytes` is empty or begins with an unrecognized control byte.
#[must_use]
pub fn parse_key_bytes(bytes: &[u8]) -> Option<(Key, usize)> {
    let &first = bytes.first()?;
    match first {
        0x03 => Some((Key::CtrlC, 1)),
        0x01 => Some((Key::CtrlA, 1)),
        0x0E => Some((Key::Down, 1)),
        0x10 => Some((Key::Up, 1)),
        b'\r' | b'\n' => Some((Key::Enter, 1)),
        b'\t' => Some((Key::Tab, 1)),
        0x7F | 0x08 => Some((Key::Backspace, 1)),
        0x1B => Some(parse_esc_sequence(bytes)),
        b if b < 0x20 => None,
        _ => decode_utf8_prefix(bytes).map(|(ch, len)| (Key::Char(ch), len)),
    }
}
