//! Dedicated console I/O boundary for the `garde-fou` binary.
//!
//! Standard stream writes (`stdout`, `stderr`), capability probes, and ephemeral TTY spinners live here.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::tui::WizardTriggerEnv;
use crate::ui::{ColorPolicy, TermCapabilities, spinner_frame};

/// Writes a formatted message line to `stdout` (unless empty, e.g. after interactive TUI).
#[expect(clippy::print_stdout, reason = "CLI reporter boundary")]
pub fn write_stdout(message: &str) {
    if !message.is_empty() {
        println!("{message}");
    }
}

/// Writes an error or diagnostic message line to `stderr`.
#[expect(clippy::print_stderr, reason = "CLI reporter boundary")]
pub fn write_stderr(message: &str) {
    eprintln!("{message}");
}

/// Captures live environment and `stdout` TTY signals to resolve `ColorPolicy`.
#[must_use]
pub fn probe_stdout_capabilities() -> TermCapabilities {
    let ci_val = env::var("CI").ok();
    let is_ci = ci_val
        .as_deref()
        .is_some_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"));
    TermCapabilities {
        is_tty: io::stdout().is_terminal(),
        is_ci,
        no_color: env::var("NO_COLOR").ok(),
        term: env::var("TERM").ok(),
        clicolor_force: env::var("CLICOLOR_FORCE").ok(),
        colorterm: env::var("COLORTERM").ok(),
    }
}

/// Captures `stdin` + `stdout` TTY state and `CI` flag for `garde init` wizard gating.
#[must_use]
pub fn probe_wizard_trigger_env() -> WizardTriggerEnv {
    let caps = probe_stdout_capabilities();
    WizardTriggerEnv {
        is_interactive_tty: io::stdin().is_terminal() && caps.is_tty,
        is_ci: caps.is_ci,
    }
}

fn write_ephemeral_stderr(bytes: &[u8]) {
    let mut err = io::stderr().lock();
    if err.write_all(bytes).is_ok() {
        err.flush().ok();
    }
}

/// Displays an ephemeral Braille spinner on `stderr` during network operations on interactive TTYs.
pub fn with_ephemeral_spinner<T, F>(animate: bool, label: &str, policy: ColorPolicy, work: F) -> T
where
    F: FnOnce() -> T,
{
    if !animate || policy == ColorPolicy::Plain || !io::stderr().is_terminal() {
        return work();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop_bg = Arc::clone(&stop);
    let msg = String::from(label);
    let handle = thread::spawn(move || {
        let mut tick = 0;
        while !stop_bg.load(Ordering::Relaxed) {
            let line = format!("\r\x1b[2K{}", spinner_frame(tick, &msg, policy));
            write_ephemeral_stderr(line.as_bytes());
            tick += 1;
            thread::sleep(Duration::from_millis(80));
        }
    });
    let res = work();
    stop.store(true, Ordering::Relaxed);
    handle.join().ok();
    write_ephemeral_stderr(b"\r\x1b[2K");
    res
}
