//! Pure Lipgloss view renderer (`render_app`) for the 3-tab `garde tui` cockpit.

use super::app_model::AppModel;
use super::app_types::{AppTab, WatchState};
use super::cache_tab::render_cache_tab;
use super::config_tab_view::render_config_tab;
use super::explorer_view::render_explorer;
use crate::ui::{ColorPolicy, PillRole, Role, paint, paint_bold, pill};

const SPINNER_FRAMES: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];

fn render_tab_bar(model: &AppModel, policy: ColorPolicy) -> String {
    let tabs: Vec<String> = AppTab::ALL
        .iter()
        .map(|&tab| {
            let role = if tab == model.active_tab {
                PillRole::BadgeAccent
            } else {
                PillRole::BadgeMuted
            };
            pill(tab.label(), role, policy)
        })
        .collect();

    let watch_badge = match model.watch_state {
        WatchState::Live => {
            let idx = usize::try_from(model.tick_count % 4).unwrap_or(0);
            let spin = SPINNER_FRAMES[idx];
            pill(&format!("{spin} LIVE"), PillRole::BadgeOk, policy)
        }
        WatchState::Paused => pill("⏸ PAUSE", PillRole::BadgeWarn, policy),
    };

    let unsaved = model.config.unsaved_changes();
    let dirty_badge = if unsaved > 0 {
        format!(
            "  {}",
            paint_bold(&format!("● {unsaved} modif."), Role::Warning, policy)
        )
    } else {
        String::new()
    };

    format!("{}  {}{dirty_badge}", tabs.join(" "), watch_badge)
}

/// Renders the entire 3-tab `garde tui` cockpit as a pure deterministic string.
#[must_use]
pub fn render_app(model: &AppModel, policy: ColorPolicy, max_width: usize) -> String {
    let header = render_tab_bar(model, policy);
    let watch_line = model
        .last_watch_summary
        .as_deref()
        .map_or_else(String::new, |delta| {
            format!("\n  {}", paint(&format!("Δ {delta}"), Role::Muted, policy))
        });

    let body = match model.active_tab {
        AppTab::Diagnostics => render_explorer(&model.explorer, policy, max_width),
        AppTab::Config => render_config_tab(&model.config, policy, max_width),
        AppTab::Cache => render_cache_tab(&model.cache, policy, max_width),
    };

    let nav_hint = paint(
        "  [Tab/Shift+Tab/1-3] onglets · [w] Live/Pause watch · [q] quitter",
        Role::Muted,
        policy,
    );

    format!("{header}{watch_line}\n\n{body}\n{nav_hint}")
}
