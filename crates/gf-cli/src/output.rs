//! Dedicated console I/O boundary for the `garde-fou` binary.
//!
//! All `stdout` and `stderr` writes in the entire workspace are isolated to this module.

/// Writes a formatted message line to `stdout`.
#[expect(clippy::print_stdout, reason = "CLI reporter boundary")]
pub fn write_stdout(message: &str) {
    println!("{message}");
}

/// Writes an error or diagnostic message line to `stderr`.
#[expect(clippy::print_stderr, reason = "CLI reporter boundary")]
pub fn write_stderr(message: &str) {
    eprintln!("{message}");
}
