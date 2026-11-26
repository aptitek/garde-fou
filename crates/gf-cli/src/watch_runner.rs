//! Debounced polling watcher (`WatchPoller`) and `garde check --watch` live terminal loop.
//!
//! All TTY writes and raw mode management are delegated to [`crate::tui::terminal`].

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use gf_engine::{
    CheckOptions, EngineError, WatchDelta, WorkspaceSnapshot, capture_snapshot, diff_snapshots,
    load_workspace_config, run_check_with_options,
};

use crate::args::OutputFormat;
use crate::reporter::render_check_report_rich;
use crate::source_loader::{load_diagnosed_sources, read_lcov_tenths};
use crate::tui::Key;
use crate::tui::terminal::{RawTermGuard, detect_term_width, poll_key_timeout, write_tty};
use crate::ui::{ColorPolicy, HumanCheckMeta, Role, paint, paint_bold};

const WATCH_POLL_MS: u64 = 250;
const WATCH_DEBOUNCE_MS: u64 = 120;

fn take_snapshot(cwd: &Path) -> WorkspaceSnapshot {
    let Ok(loaded) = load_workspace_config(cwd) else {
        return WorkspaceSnapshot::default();
    };
    capture_snapshot(cwd, &loaded.analyzer, &loaded.cache_dir).unwrap_or_default()
}

/// Metadata-fingerprint workspace poller with 250 ms polling and 120 ms debounce.
pub struct WatchPoller {
    last_poll: Instant,
    baseline: WorkspaceSnapshot,
    pending_delta: Option<(WatchDelta, Instant)>,
}

impl WatchPoller {
    pub fn new(cwd: &Path) -> Self {
        Self {
            last_poll: Instant::now(),
            baseline: take_snapshot(cwd),
            pending_delta: None,
        }
    }

    pub fn poll(&mut self, cwd: &Path) -> Option<WatchDelta> {
        let now = Instant::now();
        if now.duration_since(self.last_poll) >= Duration::from_millis(WATCH_POLL_MS) {
            self.last_poll = now;
            let current = take_snapshot(cwd);
            if let Some(delta) = diff_snapshots(&self.baseline, &current) {
                self.baseline = current;
                self.pending_delta = Some((delta, now));
            }
        }
        if let Some((_, first_seen)) = &self.pending_delta
            && now.duration_since(*first_seen) >= Duration::from_millis(WATCH_DEBOUNCE_MS)
        {
            return self.pending_delta.take().map(|(d, _)| d);
        }
        None
    }

    pub fn resync(&mut self, cwd: &Path) {
        self.baseline = take_snapshot(cwd);
        self.pending_delta = None;
        self.last_poll = Instant::now();
    }
}

enum WatchTrigger {
    Quit,
    Reload(String),
}

fn wait_for_watch_trigger(
    cwd: &Path,
    poller: &mut WatchPoller,
) -> Result<WatchTrigger, EngineError> {
    loop {
        if let Some(key) = poll_key_timeout(120)? {
            match key {
                Key::CtrlC | Key::Char('q') | Key::Esc => return Ok(WatchTrigger::Quit),
                Key::Char('r') => {
                    return Ok(WatchTrigger::Reload(String::from("rechargement manuel")));
                }
                _ => {}
            }
        }
        if let Some(delta) = poller.poll(cwd) {
            return Ok(WatchTrigger::Reload(delta.summary_short()));
        }
    }
}

fn render_watch_frame(
    cwd: &Path,
    options: &CheckOptions,
    last_delta: Option<&str>,
    policy: ColorPolicy,
) -> Result<(String, bool), EngineError> {
    let t0 = Instant::now();
    let report = run_check_with_options(cwd, options)?;
    let elapsed_ms = t0.elapsed().as_millis();
    let is_clean = report.is_clean();
    let sources = load_diagnosed_sources(cwd, &report);
    let meta = HumanCheckMeta {
        elapsed_ms,
        lcov_tenths: read_lcov_tenths(cwd, options.lcov_path.as_deref()),
        policy,
        max_width: detect_term_width(),
    };
    let body = render_check_report_rich(&report, OutputFormat::Human, &sources, &meta);
    let banner = paint_bold(
        "👀 Surveillance active (poll 250 ms, debounce 120 ms) · [r] relancer · [q/Ctrl+C] quitter",
        Role::Accent,
        policy,
    );
    let delta_line = last_delta.map_or_else(String::new, |d| {
        format!("\n  {}", paint(&format!("Δ {d}"), Role::Muted, policy))
    });
    let frame = format!("{banner}{delta_line}\n\n{body}").replace('\n', "\r\n");
    Ok((frame, is_clean))
}

/// Runs the `garde check --watch` live streaming terminal loop.
pub fn run_watch_check_loop(
    cwd: &Path,
    options: &CheckOptions,
    policy: ColorPolicy,
) -> Result<ExitCode, EngineError> {
    let _guard = RawTermGuard::enter(true)?;
    let mut poller = WatchPoller::new(cwd);
    let mut last_delta: Option<String> = None;
    loop {
        let (frame, is_clean) = render_watch_frame(cwd, options, last_delta.as_deref(), policy)?;
        write_tty(b"\x1b[H\x1b[J");
        write_tty(frame.as_bytes());

        match wait_for_watch_trigger(cwd, &mut poller)? {
            WatchTrigger::Quit => {
                return Ok(if is_clean {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                });
            }
            WatchTrigger::Reload(summary) => last_delta = Some(summary),
        }
    }
}
