//! Imperative event-loop runners for the 3-tab live cockpit (`garde tui` / `garde config`)
//! and interactive `garde uninstall`.
//!
//! All TTY writes and raw mode management are delegated to [`crate::tui::terminal`].

use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use gf_engine::{
    CheckOptions, EngineError, SkillsFetchMode, apply_project_settings, cache_clean, cache_prune,
    cache_status, load_project_settings, run_check_with_options,
};

use crate::reporter::render_agent_markdown;
use crate::source_loader::load_diagnosed_sources;
use crate::tui::terminal::{
    RawTermGuard, detect_term_width, erase_inline_lines, poll_key_timeout, read_next_key, write_tty,
};
use crate::tui::{
    AppEffect, AppEvent, AppInit, AppModel, AppTab, ExplorerSnapshot, UninstallChoice,
    UninstallWizardModel, UninstallWizardOutcome, WatchState, render_app, render_uninstall_wizard,
};
use crate::ui::ColorPolicy;
use crate::watch_runner::WatchPoller;

/// Parameters for launching the 3-tab interactive cockpit.
pub struct AppRunParams<'a> {
    pub initial_tab: AppTab,
    pub initial_opts: &'a CheckOptions,
    pub fetch_mode: SkillsFetchMode,
    pub policy: ColorPolicy,
}

/// Captures a complete [`ExplorerSnapshot`] (check report + diagnosed sources + agent markdown).
fn collect_explorer_snapshot(
    cwd: &Path,
    options: &CheckOptions,
) -> Result<ExplorerSnapshot, EngineError> {
    let t0 = Instant::now();
    let report = run_check_with_options(cwd, options)?;
    let elapsed_ms = t0.elapsed().as_millis();
    let sources = load_diagnosed_sources(cwd, &report);
    let agent_markdown = render_agent_markdown(&report);
    Ok(ExplorerSnapshot {
        report,
        sources,
        elapsed_ms,
        agent_markdown,
    })
}

struct EffectCtx<'a> {
    cwd: &'a Path,
    reload_opts: &'a CheckOptions,
    fetch_mode: &'a SkillsFetchMode,
}

fn reload_into_model(model: &mut AppModel, cwd: &Path, opts: &CheckOptions) {
    if let Ok(snap) = collect_explorer_snapshot(cwd, opts)
        && let Ok(cr) = cache_status(cwd)
    {
        model.apply_reload(snap, cr);
    }
}

fn apply_config_effect(
    model: &mut AppModel,
    draft: &gf_core::ProjectSettingsState,
    ctx: &EffectCtx<'_>,
) {
    if apply_project_settings(ctx.cwd, draft, ctx.fetch_mode.clone()).is_err() {
        return;
    }
    if let Ok(reloaded) = load_project_settings(ctx.cwd) {
        model.apply_saved_config(reloaded);
    }
    reload_into_model(model, ctx.cwd, ctx.reload_opts);
}

fn run_cache_prune_effect(model: &mut AppModel, cwd: &Path) {
    if let Ok(summary) = cache_prune(cwd)
        && let Ok(cr) = cache_status(cwd)
    {
        model.apply_cache_op("prune", summary, cr);
    }
}

fn run_cache_clean_effect(model: &mut AppModel, ctx: &EffectCtx<'_>) {
    if let Ok(summary) = cache_clean(ctx.cwd)
        && let Ok(cr) = cache_status(ctx.cwd)
    {
        model.apply_cache_op("clean", summary, cr);
        let mut cold = ctx.reload_opts.clone();
        cold.no_cache = true;
        reload_into_model(model, ctx.cwd, &cold);
    }
}

fn execute_app_effect(model: &mut AppModel, effect: AppEffect, ctx: &EffectCtx<'_>) -> bool {
    match effect {
        AppEffect::Redraw => return false,
        AppEffect::Quit => return true,
        AppEffect::CopyOsc52(seq) => write_tty(seq.as_bytes()),
        AppEffect::ReloadCheck => reload_into_model(model, ctx.cwd, ctx.reload_opts),
        AppEffect::ApplyConfig(draft) => apply_config_effect(model, &draft, ctx),
        AppEffect::PruneCache => run_cache_prune_effect(model, ctx.cwd),
        AppEffect::CleanCache => run_cache_clean_effect(model, ctx),
    }
    false
}

fn step_app_loop(
    model: &mut AppModel,
    poller: &mut WatchPoller,
    ctx: &EffectCtx<'_>,
) -> io::Result<bool> {
    if let Some(key) = poll_key_timeout(120)? {
        let effect = model.update(AppEvent::Key(key));
        let resync = matches!(
            effect,
            AppEffect::ApplyConfig(_) | AppEffect::PruneCache | AppEffect::CleanCache
        );
        let should_quit = execute_app_effect(model, effect, ctx);
        if resync {
            poller.resync(ctx.cwd);
        }
        return Ok(should_quit);
    }
    if model.watch_state() == WatchState::Live
        && let Some(delta) = poller.poll(ctx.cwd)
    {
        let eff = model.update(AppEvent::FileChanged(delta));
        execute_app_effect(model, eff, ctx);
    } else {
        model.update(AppEvent::Tick);
    }
    Ok(false)
}

/// Runs the 3-tab live Bubble Tea cockpit (`garde tui`, `garde check -i`, `garde config`).
pub fn run_app_interactive(cwd: &Path, params: &AppRunParams<'_>) -> Result<ExitCode, EngineError> {
    let initial_snap = collect_explorer_snapshot(cwd, params.initial_opts)?;
    let settings = load_project_settings(cwd)?;
    let mut model = AppModel::new(AppInit {
        initial_tab: params.initial_tab,
        snapshot: initial_snap,
        settings,
        cache_report: cache_status(cwd)?,
    });
    let mut reload_opts = params.initial_opts.clone();
    reload_opts.no_cache = false;
    let ctx = EffectCtx {
        cwd,
        reload_opts: &reload_opts,
        fetch_mode: &params.fetch_mode,
    };

    let _guard = RawTermGuard::enter(true)?;
    let mut poller = WatchPoller::new(cwd);
    loop {
        let view = render_app(&model, params.policy, detect_term_width()).replace('\n', "\r\n");
        write_tty(b"\x1b[H\x1b[J");
        write_tty(view.as_bytes());
        if step_app_loop(&mut model, &mut poller, &ctx)? {
            return Ok(if model.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    }
}

/// Runs the inline Gum-style 3-choice `garde uninstall` wizard.
pub fn run_uninstall_wizard_interactive(
    policy: ColorPolicy,
) -> io::Result<Option<UninstallChoice>> {
    let _guard = RawTermGuard::enter(false)?;
    let mut model = UninstallWizardModel::new();
    let mut prev_lines = 0;
    loop {
        let view = render_uninstall_wizard(&model, policy, detect_term_width());
        erase_inline_lines(prev_lines);
        write_tty(view.replace('\n', "\r\n").as_bytes());
        prev_lines = view.lines().count().max(1);

        match model.handle_key(read_next_key()?) {
            UninstallWizardOutcome::Continue => {}
            UninstallWizardOutcome::Confirmed(choice) => {
                erase_inline_lines(prev_lines);
                return Ok(Some(choice));
            }
            UninstallWizardOutcome::Cancelled => {
                erase_inline_lines(prev_lines);
                return Ok(None);
            }
        }
    }
}
