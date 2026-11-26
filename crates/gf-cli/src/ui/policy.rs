//! Terminal capability detection (`ColorPolicy`, `TermCapabilities`) and `OSC 52`
//! Base64 clipboard sequence encoding.

use crate::args::ColorChoice;

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Resolved terminal color rendering capability.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorPolicy {
    /// Monochrome output with zero `\x1b` escape bytes (`NO_COLOR`, `CI`, non-TTY, `--color never`).
    #[default]
    Plain,
    /// Standard 16/256-color ANSI SGR output legible on both dark and light backgrounds.
    Ansi16,
    /// 24-bit RGB tonal Lipgloss output (`COLORTERM=truecolor` or `24bit`).
    TrueColor,
}

/// Pure snapshot of terminal environment signals inspected to choose a [`ColorPolicy`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TermCapabilities {
    /// Whether the target output stream is an interactive TTY.
    pub is_tty: bool,
    /// Whether `CI` is set to a truthy value (`true` / `1`).
    pub is_ci: bool,
    /// Raw value of `NO_COLOR` if present in the environment.
    pub no_color: Option<String>,
    /// Raw value of `TERM` if present in the environment.
    pub term: Option<String>,
    /// Raw value of `CLICOLOR_FORCE` if present in the environment.
    pub clicolor_force: Option<String>,
    /// Raw value of `COLORTERM` if present in the environment.
    pub colorterm: Option<String>,
}

fn rich_policy(caps: &TermCapabilities) -> ColorPolicy {
    let is_truecolor = caps
        .colorterm
        .as_deref()
        .is_some_and(|ct| ct.eq_ignore_ascii_case("truecolor") || ct.eq_ignore_ascii_case("24bit"));
    if is_truecolor {
        ColorPolicy::TrueColor
    } else {
        ColorPolicy::Ansi16
    }
}

/// Resolves the active [`ColorPolicy`] from `--color` and pure [`TermCapabilities`].
#[must_use]
pub fn resolve_color_policy(choice: ColorChoice, caps: &TermCapabilities) -> ColorPolicy {
    match choice {
        ColorChoice::Never => ColorPolicy::Plain,
        ColorChoice::Always => rich_policy(caps),
        ColorChoice::Auto => resolve_auto_policy(caps),
    }
}

fn resolve_auto_policy(caps: &TermCapabilities) -> ColorPolicy {
    if caps.no_color.as_deref().is_some_and(|v| !v.is_empty()) {
        return ColorPolicy::Plain;
    }
    if caps
        .clicolor_force
        .as_deref()
        .is_some_and(|v| !v.is_empty() && v != "0")
    {
        return rich_policy(caps);
    }
    if !caps.is_tty || caps.is_ci || caps.term.as_deref() == Some("dumb") {
        return ColorPolicy::Plain;
    }
    rich_policy(caps)
}

/// Encodes arbitrary bytes using standard RFC 4648 Base64.
#[must_use]
pub fn encode_base64(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(BASE64_TABLE[((triple >> 18) & 0x3F) as usize]));
        out.push(char::from(BASE64_TABLE[((triple >> 12) & 0x3F) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(BASE64_TABLE[((triple >> 6) & 0x3F) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(BASE64_TABLE[(triple & 0x3F) as usize])
        } else {
            '='
        });
    }
    out
}

/// Builds a standard `OSC 52` terminal clipboard escape sequence for `text`.
#[must_use]
pub fn osc52_clipboard_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", encode_base64(text.as_bytes()))
}
