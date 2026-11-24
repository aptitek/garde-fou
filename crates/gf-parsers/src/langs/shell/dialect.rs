//! Shell dialect detection (`PosixSh` vs `Bash`) from `# shellcheck shell=...`, shebangs, and paths,
//! plus extensionless shebang language classification.

use gf_core::{Language, RepoPath, ShellDialect};

fn parse_interpreter_from_shebang(first_line: &str) -> Option<&str> {
    let rest = first_line.trim().strip_prefix("#!")?.trim();
    let mut parts = rest.split_whitespace();
    let first_prog = parts.next()?;
    let base = first_prog.rsplit('/').next().unwrap_or(first_prog);
    if base == "env" {
        for tok in parts {
            if tok.starts_with('-') || tok.contains('=') {
                continue;
            }
            return Some(tok.rsplit('/').next().unwrap_or(tok));
        }
        None
    } else {
        Some(base)
    }
}

fn map_shell_name(name: &str) -> Option<ShellDialect> {
    let lower = name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "sh" | "dash" | "ash" | "busybox" | "posix" => Some(ShellDialect::PosixSh),
        "bash" | "ksh" | "zsh" | "bats" => Some(ShellDialect::Bash),
        _ => None,
    }
}

fn find_shellcheck_shell_directive(source: &str) -> Option<ShellDialect> {
    for line in source.lines().take(30) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("#!") {
            continue;
        }
        let Some(comment) = trimmed.strip_prefix('#') else {
            break;
        };
        let norm: String = comment.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = norm.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("shellcheck ") else {
            continue;
        };
        for item in rest.split_whitespace() {
            if let Some(val) = item.strip_prefix("shell=")
                && let Some(dialect) = map_shell_name(val)
            {
                return Some(dialect);
            }
        }
    }
    None
}

const BASH_EXTENSIONS: &[&str] = &["bash", "zsh", "ksh", "bats"];
const SOURCED_SUFFIXES: &[&str] = &[".lib.sh", "_lib.sh", ".inc.sh"];

/// Detects the [`ShellDialect`] (`PosixSh` vs `Bash`) of `source` at `path`.
#[must_use]
pub fn detect_shell_dialect(path: &RepoPath, source: &str) -> ShellDialect {
    if let Some(directive_dialect) = find_shellcheck_shell_directive(source) {
        return directive_dialect;
    }
    if let Some(first_line) = source.lines().next()
        && let Some(interp) = parse_interpreter_from_shebang(first_line)
        && let Some(shebang_dialect) = map_shell_name(interp)
    {
        return shebang_dialect;
    }
    if path
        .extension()
        .is_some_and(|ext| BASH_EXTENSIONS.iter().any(|c| ext.eq_ignore_ascii_case(c)))
    {
        ShellDialect::Bash
    } else {
        ShellDialect::PosixSh
    }
}

/// Returns `true` when `source` at `path` is a sourced shell library rather than an executable script.
#[must_use]
pub fn is_sourced_shell_library(path: &RepoPath, source: &str) -> bool {
    let has_shebang = source
        .lines()
        .next()
        .is_some_and(|l| l.trim_start().starts_with("#!"));
    if has_shebang {
        return false;
    }
    if find_shellcheck_shell_directive(source).is_some() {
        return true;
    }
    let name = path.file_name().to_ascii_lowercase();
    SOURCED_SUFFIXES.iter().any(|suf| name.ends_with(suf))
        || matches!(
            name.as_str(),
            ".bashrc" | ".bash_profile" | ".profile" | "env.sh"
        )
}

/// Inspects line 1 of an extensionless file to detect `sh`/`bash` or `pwsh` shebangs.
#[must_use]
pub fn detect_shebang_language(first_line: &str) -> Option<Language> {
    let interp = parse_interpreter_from_shebang(first_line)?;
    if map_shell_name(interp).is_some() {
        return Some(Language::Shell);
    }
    if interp.eq_ignore_ascii_case("pwsh") || interp.eq_ignore_ascii_case("powershell") {
        return Some(Language::PowerShell);
    }
    None
}
