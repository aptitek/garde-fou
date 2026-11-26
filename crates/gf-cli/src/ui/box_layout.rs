//! Pure Lipgloss layout primitives (`rounded_card`, `join_horizontal_top`,
//! `status_tree`, `kv_row`, and `gauge_bar`) guaranteeing exact column alignment.

use core::fmt::Write as _;

use super::ansi::{truncate_visible, visible_width};
use super::policy::ColorPolicy;
use super::theme::{Role, paint, paint_bold};

/// Border role and color policy bundled for [`rounded_card`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardStyle {
    /// Semantic color role applied to card borders and optional title.
    pub border_role: Role,
    /// Active terminal color policy.
    pub policy: ColorPolicy,
}

impl CardStyle {
    /// Constructs a [`CardStyle`] from `border_role` and `policy`.
    #[must_use]
    pub const fn new(border_role: Role, policy: ColorPolicy) -> Self {
        Self {
            border_role,
            policy,
        }
    }
}

/// Tree item marker for `garde init`, `garde sync`, and `garde lock` reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeSymbol {
    /// Newly created artifact (`+`).
    Created,
    /// Updated / merged artifact (`~`).
    Updated,
    /// Unchanged existing artifact (`=`).
    Intact,
    /// Cryptographically sealed lockfile (`*`).
    Sealed,
    /// Non-fatal warning (`!`).
    Warn,
}

fn sanitize_single_line(raw: &str) -> String {
    raw.replace('\t', "    ").replace(['\r', '\n'], " ")
}

fn expand_card_lines(lines: &[String]) -> Vec<String> {
    let mut rows = Vec::with_capacity(lines.len());
    for entry in lines {
        let clean = entry.replace('\r', "").replace('\t', "    ");
        for sub in clean.split('\n') {
            rows.push(String::from(sub));
        }
    }
    rows
}

fn render_top_edge(title: Option<&str>, outer_width: usize, style: CardStyle) -> String {
    let clean_title = title
        .map(sanitize_single_line)
        .map(|s| String::from(s.trim()))
        .filter(|s| !s.is_empty());
    if let Some(raw_t) = clean_title {
        let t_trunc = truncate_visible(&raw_t, outer_width.saturating_sub(6));
        let t_vis = visible_width(&t_trunc);
        if t_vis > 0 {
            let dashes = outer_width.saturating_sub(t_vis + 5);
            let left = paint("╭─ ", style.border_role, style.policy);
            let mid = paint_bold(&t_trunc, style.border_role, style.policy);
            let right = paint(
                &format!(" {}╮", "─".repeat(dashes)),
                style.border_role,
                style.policy,
            );
            return format!("{left}{mid}{right}");
        }
    }
    paint(
        &format!("╭{}╮", "─".repeat(outer_width - 2)),
        style.border_role,
        style.policy,
    )
}

/// Renders a Lipgloss rounded box (`╭─ Title ───╮` .. `╰───────────╯`) whose lines
/// all have the exact same [`visible_width`], clamped to `max_width` (`>= 8`).
#[must_use]
pub fn rounded_card(
    title: Option<&str>,
    lines: &[String],
    style: CardStyle,
    max_width: usize,
) -> String {
    let outer_cap = max_width.max(8);
    let inner_cap = outer_cap - 4;
    let rows = expand_card_lines(lines);
    let max_row_vis = rows
        .iter()
        .map(|r| visible_width(r).min(inner_cap))
        .max()
        .unwrap_or(0);
    let title_min = title
        .map(sanitize_single_line)
        .map(|t| String::from(t.trim()))
        .filter(|t| !t.is_empty())
        .map_or(0, |t| {
            let trunc = truncate_visible(&t, outer_cap - 6);
            (visible_width(&trunc) + 2).min(inner_cap)
        });
    let inner_width = max_row_vis.max(title_min).max(1);
    let outer_width = inner_width + 4;

    let mut out = render_top_edge(title, outer_width, style);
    let vbar = paint("│", style.border_role, style.policy);
    for row in &rows {
        let clipped = truncate_visible(row, inner_width);
        let pad = " ".repeat(inner_width.saturating_sub(visible_width(&clipped)));
        let _ = write!(out, "\n{vbar} {clipped}{pad} {vbar}");
    }
    let bottom = paint(
        &format!("╰{}╯", "─".repeat(outer_width - 2)),
        style.border_role,
        style.policy,
    );
    let _ = write!(out, "\n{bottom}");
    out
}

/// Joins two multi-line blocks side-by-side aligned at the top with `gap` spaces between them.
#[must_use]
pub fn join_horizontal_top(left: &str, right: &str, gap: usize) -> String {
    let left_lines: Vec<&str> = left.split('\n').collect();
    let right_lines: Vec<&str> = right.split('\n').collect();
    let left_w = left_lines
        .iter()
        .map(|l| visible_width(l))
        .max()
        .unwrap_or(0);
    let height = left_lines.len().max(right_lines.len());
    let spacer = " ".repeat(gap);
    let mut rows = Vec::with_capacity(height);
    for idx in 0..height {
        let l = left_lines.get(idx).copied().unwrap_or("");
        let r = right_lines.get(idx).copied().unwrap_or("");
        let pad = " ".repeat(left_w.saturating_sub(visible_width(l)));
        if r.is_empty() {
            rows.push(format!("{l}{pad}"));
        } else {
            rows.push(format!("{l}{pad}{spacer}{r}"));
        }
    }
    rows.join("\n")
}

/// Renders a visual hierarchy (`├─` / `╰─`) under `header`.
#[must_use]
pub fn status_tree(header: &str, entries: &[(TreeSymbol, String)], policy: ColorPolicy) -> String {
    let mut out = String::from(header);
    let last_idx = entries.len().saturating_sub(1);
    for (idx, (sym, text)) in entries.iter().enumerate() {
        let branch = if idx == last_idx { "╰─" } else { "├─" };
        let conn = paint(branch, Role::SurfaceBorder, policy);
        let badge = render_tree_symbol(*sym, policy);
        let _ = write!(out, "\n  {conn} {badge} {text}");
    }
    out
}

fn render_tree_symbol(sym: TreeSymbol, policy: ColorPolicy) -> String {
    match sym {
        TreeSymbol::Created => paint_bold("+", Role::Success, policy),
        TreeSymbol::Updated => paint_bold("~", Role::Accent, policy),
        TreeSymbol::Intact => paint("=", Role::Muted, policy),
        TreeSymbol::Sealed => paint_bold("🔒", Role::Success, policy),
        TreeSymbol::Warn => paint_bold("!", Role::Warning, policy),
    }
}

/// Formats an aligned `key  value` row with muted key padding.
#[must_use]
pub fn kv_row(key: &str, value: &str, key_width: usize, policy: ColorPolicy) -> String {
    let pad = " ".repeat(key_width.saturating_sub(visible_width(key)));
    format!("{}{pad}  {value}", paint(key, Role::Muted, policy))
}

/// Renders a compact coverage/mutation gauge (`━━━━━━━━░░ 94.5%`) from tenths of a percent (`0..=1000`).
#[must_use]
pub fn gauge_bar(pct_tenths: u32, bar_width: usize, policy: ColorPolicy) -> String {
    let clamped = pct_tenths.min(1000);
    let width = bar_width.max(4);
    let filled = ((clamped as usize) * width + 500) / 1000;
    let filled = filled.min(width);
    let empty = width - filled;
    let role = if clamped >= 850 {
        Role::Success
    } else if clamped >= 700 {
        Role::Warning
    } else {
        Role::Error
    };
    let active = paint(&"━".repeat(filled), role, policy);
    let rest = paint(&"░".repeat(empty), Role::Muted, policy);
    let whole = clamped / 10;
    let frac = clamped % 10;
    format!("{active}{rest} {whole}.{frac}%")
}
