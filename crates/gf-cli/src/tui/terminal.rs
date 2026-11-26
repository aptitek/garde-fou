//! Imperative raw TTY boundary (`RawTermGuard`, raw key reader with poll timeout,
//! inline/alt-screen primitives, and `OSC 52` clipboard emission).
//!
//! Together with `crate::output`, this is the sole module permitted to touch real TTY handles.

use std::env;
use std::io::{self, Read, Write};

use super::keys::{Key, parse_key_bytes};
use super::wizard_state::WizardModel;
use super::wizard_types::{WizardOutcome, WizardTransition};
use super::wizard_view::render_wizard;
use crate::ui::ColorPolicy;

pub(crate) fn write_tty(bytes: &[u8]) {
    let mut err = io::stderr().lock();
    if err.write_all(bytes).is_ok() {
        err.flush().ok();
    }
}

/// Queries the current terminal column width (clamped to `40..=140`, defaulting to `100`).
#[must_use]
pub fn detect_term_width() -> usize {
    #[cfg(unix)]
    if let Ok(ws) = rustix::termios::tcgetwinsize(io::stderr())
        && ws.ws_col > 0
    {
        return usize::from(ws.ws_col).clamp(40, 140);
    }
    #[cfg(not(unix))]
    {
        let _keep_rustix_linked = rustix::io::Errno::INTR;
    }
    env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map_or(100, |c| c.clamp(40, 140))
}

/// RAII raw TTY guard that unconditionally restores cursor visibility and terminal mode on drop.
pub(crate) struct RawTermGuard {
    #[cfg(unix)]
    saved: Option<rustix::termios::Termios>,
    alt_screen: bool,
}

impl RawTermGuard {
    pub(crate) fn enter(alt_screen: bool) -> io::Result<Self> {
        #[cfg(unix)]
        let saved = {
            let orig = rustix::termios::tcgetattr(io::stdin())
                .map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
            let mut raw = orig.clone();
            raw.make_raw();
            rustix::termios::tcsetattr(io::stdin(), rustix::termios::OptionalActions::Now, &raw)
                .map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
            Some(orig)
        };
        if alt_screen {
            write_tty(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H");
        } else {
            write_tty(b"\x1b[?25l");
        }
        Ok(Self {
            #[cfg(unix)]
            saved,
            alt_screen,
        })
    }
}

impl Drop for RawTermGuard {
    fn drop(&mut self) {
        if self.alt_screen {
            write_tty(b"\x1b[?25h\x1b[?1049l");
        } else {
            write_tty(b"\x1b[?25h");
        }
        #[cfg(unix)]
        if let Some(orig) = &self.saved {
            rustix::termios::tcsetattr(io::stdin(), rustix::termios::OptionalActions::Now, orig)
                .ok();
        }
    }
}

const fn utf8_seq_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if (first & 0xE0) == 0xC0 {
        2
    } else if (first & 0xF0) == 0xE0 {
        3
    } else {
        4
    }
}

#[cfg(unix)]
fn stdin_poll_ready(timeout_ms: u32) -> io::Result<bool> {
    let stdin = io::stdin();
    let mut fds = [rustix::event::PollFd::new(
        &stdin,
        rustix::event::PollFlags::IN,
    )];
    let timeout = rustix::event::Timespec {
        tv_sec: i64::from(timeout_ms / 1000),
        tv_nsec: i64::from(timeout_ms % 1000) * 1_000_000,
    };
    match rustix::event::poll(&mut fds, Some(&timeout)) {
        Ok(n) => Ok(n > 0),
        Err(rustix::io::Errno::INTR) => Ok(false),
        Err(e) => Err(io::Error::from_raw_os_error(e.raw_os_error())),
    }
}

#[cfg(not(unix))]
fn stdin_poll_ready(timeout_ms: u32) -> io::Result<bool> {
    std::thread::sleep(std::time::Duration::from_millis(u64::from(timeout_ms)));
    Ok(false)
}

fn read_utf8_tail(stdin: &mut io::Stdin, buf: &mut [u8; 8], need: usize) -> io::Result<usize> {
    let mut len = 1;
    while len < need {
        let got = stdin.read(&mut buf[len..need])?;
        if got == 0 {
            break;
        }
        len += got;
    }
    Ok(len)
}

fn read_packet(stdin: &mut io::Stdin) -> io::Result<Option<([u8; 8], usize)>> {
    let mut buf = [0_u8; 8];
    if stdin.read(&mut buf[..1])? == 0 {
        return Ok(None);
    }
    let len = if buf[0] == 0x1B && stdin_poll_ready(25).unwrap_or(false) {
        1 + stdin.read(&mut buf[1..6])?
    } else if buf[0] >= 0x80 {
        let need = utf8_seq_len(buf[0]);
        read_utf8_tail(stdin, &mut buf, need)?
    } else {
        1
    };
    Ok(Some((buf, len)))
}

pub(crate) fn read_next_key() -> io::Result<Key> {
    let mut stdin = io::stdin();
    loop {
        let Some((buf, len)) = read_packet(&mut stdin)? else {
            return Ok(Key::Esc);
        };
        if let Some((key, _)) = parse_key_bytes(&buf[..len]) {
            return Ok(key);
        }
    }
}

pub(crate) fn poll_key_timeout(timeout_ms: u32) -> io::Result<Option<Key>> {
    if !stdin_poll_ready(timeout_ms)? {
        return Ok(None);
    }
    read_next_key().map(Some)
}

pub(crate) fn erase_inline_lines(prev_lines: usize) {
    if prev_lines > 1 {
        write_tty(format!("\r\x1b[{}A\x1b[J", prev_lines - 1).as_bytes());
    } else if prev_lines == 1 {
        write_tty(b"\r\x1b[J");
    }
}

/// Runs the interactive Gum-style `garde init` wizard inside an inline TTY viewport.
///
/// # Errors
///
/// Returns [`io::Error`] if entering raw TTY mode or reading keyboard input fails.
pub fn run_wizard_interactive(
    mut model: WizardModel,
    policy: ColorPolicy,
) -> io::Result<WizardOutcome> {
    let _guard = RawTermGuard::enter(false)?;
    let mut prev_lines = 0;
    loop {
        let view = render_wizard(&model, policy, detect_term_width());
        erase_inline_lines(prev_lines);
        write_tty(view.replace('\n', "\r\n").as_bytes());
        prev_lines = view.lines().count().max(1);

        let key = read_next_key()?;
        if let WizardTransition::Done(outcome) = model.handle_key(key) {
            erase_inline_lines(prev_lines);
            return Ok(outcome);
        }
    }
}
