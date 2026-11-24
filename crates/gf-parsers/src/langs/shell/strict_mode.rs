//! POSIX/Bash strict-mode flag parser and `SH-STRICT-MODE` verifier (`GF080`).

use gf_core::{LangRuleFinding, ShellDialect, ShellViolation, Span};
use tree_sitter::Node;

use crate::cst::node_text;

/// Compact bitset tracking `set -e`, `set -u`, and `set -o pipefail` flags (zero struct bools).
#[derive(Clone, Copy, Default)]
pub struct StrictFlags(u8);

impl StrictFlags {
    const ERREXIT: u8 = 0b001;
    const NOUNSET: u8 = 0b010;
    const PIPEFAIL: u8 = 0b100;

    /// Returns `true` if `set -e` or `set -o errexit` was enabled.
    #[must_use]
    pub const fn has_errexit(self) -> bool {
        (self.0 & Self::ERREXIT) != 0
    }

    /// Returns `true` if `set -u` or `set -o nounset` was enabled.
    #[must_use]
    pub const fn has_nounset(self) -> bool {
        (self.0 & Self::NOUNSET) != 0
    }

    /// Returns `true` if `set -o pipefail` was enabled.
    #[must_use]
    pub const fn has_pipefail(self) -> bool {
        (self.0 & Self::PIPEFAIL) != 0
    }
}

fn apply_named_option(opt: &str, flags: &mut StrictFlags) {
    match opt {
        "errexit" => flags.0 |= StrictFlags::ERREXIT,
        "nounset" => flags.0 |= StrictFlags::NOUNSET,
        "pipefail" => flags.0 |= StrictFlags::PIPEFAIL,
        _ => {}
    }
}

/// Parses `set` command arguments (`-e`, `-u`, `-eu`, `-euo pipefail`, `-o pipefail`) into `flags`.
pub fn update_set_flags(args: &[Node<'_>], source: &str, flags: &mut StrictFlags) {
    let mut iter = args.iter().map(|n| node_text(*n, source).trim());
    while let Some(tok) = iter.next() {
        let Some(short) = tok.strip_prefix('-').filter(|s| !s.starts_with('-')) else {
            continue;
        };
        if short.contains('e') {
            flags.0 |= StrictFlags::ERREXIT;
        }
        if short.contains('u') {
            flags.0 |= StrictFlags::NOUNSET;
        }
        if short.ends_with('o')
            && let Some(opt) = iter.next()
        {
            apply_named_option(opt, flags);
        }
    }
}

/// Scans top-level `set` commands and returns active [`StrictFlags`].
#[must_use]
pub fn scan_top_level_strict_flags(root: Node<'_>, source: &str) -> StrictFlags {
    let mut flags = StrictFlags::default();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "command"
            && child
                .child_by_field_name("name")
                .is_some_and(|n| node_text(n, source).trim() == "set")
        {
            let mut c_arg = child.walk();
            let args: Vec<Node<'_>> = child
                .children_by_field_name("argument", &mut c_arg)
                .collect();
            update_set_flags(&args, source, &mut flags);
        }
    }
    flags
}

/// Emits `SH-STRICT-MODE` if `flags` lack required strictness for `dialect`.
pub fn check_strict_mode_header(
    dialect: ShellDialect,
    flags: StrictFlags,
    out: &mut Vec<LangRuleFinding>,
) {
    let ok = match dialect {
        ShellDialect::PosixSh => flags.has_errexit() && flags.has_nounset(),
        ShellDialect::Bash => flags.has_errexit() && flags.has_nounset() && flags.has_pipefail(),
    };
    if !ok {
        let expected = match dialect {
            ShellDialect::PosixSh => "set -eu",
            ShellDialect::Bash => "set -euo pipefail",
        };
        out.push(LangRuleFinding::new(
            ShellViolation::MissingStrictMode,
            Span::FIRST,
            format!("executable `{dialect}` script does not enable `{expected}`"),
        ));
    }
}
