//! Cross-platform Unix/Windows tool output path normalization and [`ToolParseError`].

use core::fmt;
use std::path::Path;

use gf_core::{ExternalToolKind, RepoPath};

/// Closed error type returned when an external tool emits malformed JSON/NDJSON (`GF082`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolParseError {
    /// Tool output could not be deserialized as valid JSON/NDJSON.
    MalformedJson {
        /// Tool identifier (`ruff`, `mypy`, `pyright`).
        tool: ExternalToolKind,
        /// Parser error message.
        detail: String,
    },
    /// Tool output contained an unnormalizable path.
    InvalidPath {
        /// Tool identifier.
        tool: ExternalToolKind,
        /// Raw path string reported by the tool.
        raw_path: String,
    },
}

impl fmt::Display for ToolParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson { tool, detail } => write!(f, "malformed {tool} JSON: {detail}"),
            Self::InvalidPath { tool, raw_path } => {
                write!(f, "invalid {tool} file path `{raw_path}`")
            }
        }
    }
}

impl std::error::Error for ToolParseError {}

fn strip_drive_letter(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        s.get(2..).unwrap_or(s)
    } else {
        s
    }
}

fn strip_root_prefix<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        return None;
    }
    let head = path.get(..root.len())?;
    if !head.eq_ignore_ascii_case(root) {
        return None;
    }
    path.get(root.len()..)
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
}

pub(super) fn normalize_tool_path(
    raw: &str,
    root: &Path,
    tool: ExternalToolKind,
) -> Result<RepoPath, ToolParseError> {
    let trimmed = raw.trim();
    let slash_owned = trimmed
        .strip_prefix("file://")
        .unwrap_or(trimmed)
        .replace('\\', "/");
    let slash_path = slash_owned
        .strip_prefix('/')
        .filter(|s| s.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(&slash_owned);

    let root_norm = root
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    let stripped = strip_root_prefix(slash_path, &root_norm).unwrap_or_else(|| {
        let no_drive = strip_drive_letter(slash_path);
        let root_no_drive = strip_drive_letter(&root_norm);
        strip_root_prefix(no_drive, root_no_drive).unwrap_or(no_drive)
    });

    let rel = stripped.trim_start_matches("./").trim_start_matches('/');
    let target = if rel.is_empty() {
        "pyproject.toml"
    } else {
        rel
    };
    RepoPath::new(target)
        .or_else(|_| RepoPath::new("pyproject.toml"))
        .map_err(|_| ToolParseError::InvalidPath {
            tool,
            raw_path: String::from(raw),
        })
}
