//! Semantic Solarized & Material 3 Tonal color palette (`Role`, `PillRole`)
//! legible across dark/light terminals with zero-escape `Plain` fallback.

use super::policy::ColorPolicy;

/// Semantic foreground styling roles inspired by Solarized & Material 3 Tonal tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Interactive focus, links, and primary highlights.
    Accent = 0,
    /// Clean check pass, created files, and positive metrics.
    Success = 1,
    /// Rule violations, failure states, and caret underlines.
    Error = 2,
    /// Non-blocking warnings and threshold proximity notices.
    Warning = 3,
    /// Line numbers, subtle metadata, and secondary labels.
    Muted = 4,
    /// Rounded card and tree connector borders.
    SurfaceBorder = 5,
    /// Source code snippet text inside numbered gutters.
    CodeLine = 6,
}

/// Semantic tonal badge (*pill*) roles with inverted/paired background & foreground contrast.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PillRole {
    /// High-contrast emerald success pill (`BadgeOk`).
    BadgeOk = 0,
    /// High-contrast crimson error / rule-code pill (`BadgeError`).
    BadgeError = 1,
    /// High-contrast amber warning pill (`BadgeWarn`).
    BadgeWarn = 2,
    /// High-contrast cyan/indigo header pill (`BadgeAccent`).
    BadgeAccent = 3,
    /// Tonal violet/slate 6-domain capsule (`BadgeDomain`).
    BadgeDomain = 4,
    /// Subtle neutral metadata capsule (`BadgeMuted`).
    BadgeMuted = 5,
}

const ANSI16_ROLES: [&str; 7] = [
    "\x1b[36m",
    "\x1b[32m",
    "\x1b[31m",
    "\x1b[33m",
    "\x1b[2m",
    "\x1b[2;36m",
    "\x1b[39m",
];

const TRUECOLOR_ROLES: [&str; 7] = [
    "\x1b[38;2;2;132;199m",
    "\x1b[38;2;22;163;74m",
    "\x1b[38;2;220;38;38m",
    "\x1b[38;2;217;119;6m",
    "\x1b[38;2;100;116;139m",
    "\x1b[38;2;71;85;105m",
    "\x1b[38;2;148;163;184m",
];

const ANSI16_PILLS: [&str; 6] = [
    "\x1b[1;30;42m",
    "\x1b[1;37;41m",
    "\x1b[1;30;43m",
    "\x1b[1;30;46m",
    "\x1b[1;37;45m",
    "\x1b[2;7m",
];

const TRUECOLOR_PILLS: [&str; 6] = [
    "\x1b[1;38;2;240;253;244;48;2;21;128;61m",
    "\x1b[1;38;2;254;242;242;48;2;185;28;28m",
    "\x1b[1;38;2;255;251;235;48;2;180;83;9m",
    "\x1b[1;38;2;240;249;255;48;2;3;105;161m",
    "\x1b[1;38;2;238;242;255;48;2;67;56;202m",
    "\x1b[1;38;2;226;232;240;48;2;51;65;85m",
];

const fn role_sgr(role: Role, policy: ColorPolicy) -> &'static str {
    match policy {
        ColorPolicy::Plain => "",
        ColorPolicy::Ansi16 => ANSI16_ROLES[role as usize],
        ColorPolicy::TrueColor => TRUECOLOR_ROLES[role as usize],
    }
}

const fn pill_sgr(role: PillRole, policy: ColorPolicy) -> &'static str {
    match policy {
        ColorPolicy::Plain => "",
        ColorPolicy::Ansi16 => ANSI16_PILLS[role as usize],
        ColorPolicy::TrueColor => TRUECOLOR_PILLS[role as usize],
    }
}

/// Applies semantic foreground color to `text` under `policy` (zero escapes when `Plain` or empty).
#[must_use]
pub fn paint(text: &str, role: Role, policy: ColorPolicy) -> String {
    if text.is_empty() || policy == ColorPolicy::Plain {
        return String::from(text);
    }
    format!("{}{text}\x1b[0m", role_sgr(role, policy))
}

/// Applies bold semantic foreground color to `text` under `policy` (zero escapes when `Plain` or empty).
#[must_use]
pub fn paint_bold(text: &str, role: Role, policy: ColorPolicy) -> String {
    if text.is_empty() || policy == ColorPolicy::Plain {
        return String::from(text);
    }
    format!("\x1b[1m{}{text}\x1b[0m", role_sgr(role, policy))
}

/// Renders a Lipgloss tonal pill (` LABEL ` in color, `[LABEL]` in `Plain`),
/// always adding `+2` visible columns to `visible_width(label)`.
#[must_use]
pub fn pill(label: &str, role: PillRole, policy: ColorPolicy) -> String {
    if policy == ColorPolicy::Plain {
        return format!("[{label}]");
    }
    format!("{} {label} \x1b[0m", pill_sgr(role, policy))
}
