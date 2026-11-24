//! Pure `#![no_std]` Conventional Commits validator (`garde commit-msg`, `GF077`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, Severity, Span};
use crate::path::RepoPath;
use crate::validation::Validation;

const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "refactor", "perf", "test", "docs", "build", "ci", "chore", "revert",
];

const VAGUE_DESCRIPTIONS: &[&str] = &[
    "wip",
    "update",
    "updates",
    "fix",
    "fixes",
    "test",
    "tests",
    "misc",
    "changes",
    "stuff",
    "fix stuff",
    "temp",
    "tmp",
];

/// `[commit]` Conventional Commit policy configuration (`GF077`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitConfig {
    /// Allowed Conventional Commit `<type>` prefixes.
    pub allowed_types: Vec<String>,
    /// Optional closed list of allowed `<scope>` identifiers (empty = any kebab-case scope).
    pub allowed_scopes: Vec<String>,
    /// Maximum character length of the first commit header line.
    pub max_header_len: usize,
}

impl CommitConfig {
    /// Returns standard Conventional Commits defaults (`max_header_len = 100`).
    #[must_use]
    pub fn strict_defaults() -> Self {
        Self {
            allowed_types: DEFAULT_TYPES.iter().map(|&s| String::from(s)).collect(),
            allowed_scopes: Vec::new(),
            max_header_len: 100,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "cm(t={:?},s={:?},h={})",
            self.allowed_types, self.allowed_scopes, self.max_header_len
        )
    }
}

impl Default for CommitConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

fn commit_diag(message: String, remediation: &str, measured: u32, threshold: u32) -> Diagnostic {
    Diagnostic {
        rule_id: String::from("GF077"),
        severity: Severity::Error,
        message,
        remediation: String::from(remediation),
        measured,
        threshold,
        path: RepoPath::commit_editmsg(),
        span: Span::FIRST,
    }
}

fn extract_header_line(raw: &str) -> Option<&str> {
    raw.lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim_start().starts_with('#'))
        .find(|l| !l.trim().is_empty())
}

fn is_valid_kebab_scope(scope: &str) -> bool {
    !scope.is_empty()
        && !scope.starts_with('-')
        && !scope.ends_with('-')
        && !scope.contains("--")
        && scope
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'/')
}

fn parse_header_prefix(prefix: &str) -> Result<(&str, Option<&str>), &'static str> {
    let without_bang = prefix.strip_suffix('!').unwrap_or(prefix);
    if let Some((commit_type, rest)) = without_bang.split_once('(') {
        let Some(scope) = rest.strip_suffix(')') else {
            return Err("unclosed `(` in commit scope; expected `<type>(<scope>): <description>`");
        };
        if commit_type.is_empty() || scope.is_empty() {
            return Err("empty commit type or scope before `:`");
        }
        Ok((commit_type, Some(scope)))
    } else if without_bang.contains(')') {
        Err("unexpected `)` in commit header prefix")
    } else if without_bang.is_empty() {
        Err("missing Conventional Commit `<type>` before `:`")
    } else {
        Ok((without_bang, None))
    }
}

fn check_type_and_scope(
    commit_type: &str,
    scope: Option<&str>,
    cfg: &CommitConfig,
    diags: &mut Vec<Diagnostic>,
) {
    if !cfg.allowed_types.iter().any(|t| t == commit_type) {
        diags.push(commit_diag(
            format!(
                "invalid commit type `{commit_type}` (allowed: {})",
                cfg.allowed_types.join(", ")
            ),
            "Prefix your commit with a Conventional Commit type such as `feat:`, `fix:`, `refactor:`, `test:`, or `docs:`.",
            1,
            0,
        ));
    }
    let Some(sc) = scope else {
        return;
    };
    if !is_valid_kebab_scope(sc) {
        diags.push(commit_diag(
            format!("commit scope `{sc}` must be lowercase kebab-case"),
            "Use a concise lowercase scope such as `feat(rules): ...` or `fix(cli): ...`.",
            1,
            0,
        ));
    } else if !cfg.allowed_scopes.is_empty() && !cfg.allowed_scopes.iter().any(|s| s == sc) {
        diags.push(commit_diag(
            format!(
                "commit scope `{sc}` is not in allowed_scopes ({})",
                cfg.allowed_scopes.join(", ")
            ),
            "Choose one of the scopes declared in `[commit] allowed_scopes` in `garde-fou.toml`.",
            1,
            0,
        ));
    }
}

fn check_description(desc: &str, diags: &mut Vec<Diagnostic>) {
    let char_len = u32::try_from(desc.chars().count()).unwrap_or(u32::MAX);
    if char_len < 5 {
        diags.push(commit_diag(
            format!("commit description `{desc}` is too short ({char_len} chars, min 5)"),
            "Write a clear imperative description of at least 5 characters after `<type>: `.",
            char_len,
            5,
        ));
        return;
    }
    let normalized = desc.trim_end_matches('.').to_ascii_lowercase();
    if VAGUE_DESCRIPTIONS.iter().any(|&v| normalized == v) {
        diags.push(commit_diag(
            format!("vague non-descriptive commit summary `{desc}` is forbidden"),
            "State specifically what changed and why instead of placeholder summaries like `wip`, `update`, or `fix stuff`.",
            1,
            0,
        ));
    }
}

/// Validates a raw Git commit message against [`CommitConfig`] (`GF077`).
#[must_use]
pub fn validate_commit_message(raw: &str, cfg: &CommitConfig) -> Validation<(), Diagnostic> {
    let Some(header) = extract_header_line(raw) else {
        return Validation::Invalid(alloc::vec![commit_diag(
            String::from("commit message is empty"),
            "Provide a Conventional Commit header: `<type>(<scope>): <imperative summary>`.",
            0,
            1,
        )]);
    };

    let mut diags = Vec::new();
    let header_len = header.chars().count();
    if header_len > cfg.max_header_len {
        diags.push(commit_diag(
            format!(
                "commit header length is {header_len} chars (max {})",
                cfg.max_header_len
            ),
            "Keep the first line concise (`<= max_header_len`) and move details into the commit body.",
            u32::try_from(header_len).unwrap_or(u32::MAX),
            u32::try_from(cfg.max_header_len).unwrap_or(u32::MAX),
        ));
    }

    let Some((prefix, after_colon)) = header.split_once(": ") else {
        diags.push(commit_diag(
            format!("header `{header}` does not match `<type>(<scope>): <description>`"),
            "Separate the Conventional Commit prefix and summary with `: ` (colon + space), e.g. `feat(rules): add tarjan cycle detector`.",
            1,
            0,
        ));
        return Validation::<(), Diagnostic>::from_errors(diags);
    };

    match parse_header_prefix(prefix) {
        Ok((commit_type, scope)) => check_type_and_scope(commit_type, scope, cfg, &mut diags),
        Err(reason) => diags.push(commit_diag(
            String::from(reason),
            "Format the prefix as `<type>` or `<type>(<scope>)` optionally followed by `!`.",
            1,
            0,
        )),
    }
    check_description(after_colon.trim(), &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
