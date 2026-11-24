//! Validated repository-relative paths.

use alloc::string::String;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Validation failures when constructing a [`RepoPath`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepoPathError {
    /// The provided path string was empty.
    Empty,
    /// The provided path started with a root separator (`/` or `\`).
    Absolute,
    /// The provided path contained a `..` parent traversal segment.
    ParentTraversal,
}

impl fmt::Display for RepoPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("repository path must not be empty"),
            Self::Absolute => f.write_str("repository path must be relative"),
            Self::ParentTraversal => f.write_str("repository path must not traverse '..'"),
        }
    }
}

impl core::error::Error for RepoPathError {}

/// Normalized, validated path relative to the repository root.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    /// Validates and normalizes a repository-relative path.
    ///
    /// # Errors
    ///
    /// Returns [`RepoPathError`] if `raw` is empty, absolute, or contains `..` segments.
    pub fn new(raw: &str) -> Result<Self, RepoPathError> {
        if raw.is_empty() {
            return Err(RepoPathError::Empty);
        }
        let has_windows_drive = raw.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && raw.as_bytes().get(1) == Some(&b':');
        if raw.starts_with('/') || raw.starts_with('\\') || has_windows_drive {
            return Err(RepoPathError::Absolute);
        }
        if raw.split(['/', '\\']).any(|segment| segment == "..") {
            return Err(RepoPathError::ParentTraversal);
        }
        Ok(Self(raw.replace('\\', "/")))
    }

    /// Borrows the normalized forward-slash path slice.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the canonical repository path for `garde-fou.toml`.
    #[must_use]
    pub fn manifest() -> Self {
        Self(String::from("garde-fou.toml"))
    }

    /// Returns the canonical repository path for `AGENTS.md`.
    #[must_use]
    pub fn agents_md() -> Self {
        Self(String::from("AGENTS.md"))
    }

    /// Returns the canonical pseudo-path for `.git/COMMIT_EDITMSG`.
    #[must_use]
    pub fn commit_editmsg() -> Self {
        Self(String::from("COMMIT_EDITMSG"))
    }

    /// Returns the final path component (file name).
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(self.0.as_str())
    }

    /// Returns the file extension without the leading dot, if present.
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        let (stem, ext) = name.rsplit_once('.')?;
        if stem.is_empty() || ext.is_empty() {
            None
        } else {
            Some(ext)
        }
    }
}

impl TryFrom<String> for RepoPath {
    type Error = RepoPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<RepoPath> for String {
    fn from(path: RepoPath) -> Self {
        path.0
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};

    use super::{RepoPath, RepoPathError};

    #[test]
    fn validates_and_extracts_path_metadata() -> Result<(), RepoPathError> {
        let path = RepoPath::try_from(String::from("crates\\gf-core\\src/lib.rs"))?;
        assert_eq!(path.as_str(), "crates/gf-core/src/lib.rs");
        assert_eq!(path.to_string(), "crates/gf-core/src/lib.rs");
        assert_eq!(path.file_name(), "lib.rs");
        assert_eq!(path.extension(), Some("rs"));

        let dotfile = RepoPath::new(".gitignore")?;
        assert_eq!(dotfile.extension(), None);
        let raw: String = dotfile.into();
        assert_eq!(raw, ".gitignore");
        Ok(())
    }

    #[test]
    fn rejects_invalid_paths() {
        assert_eq!(RepoPath::new(""), Err(RepoPathError::Empty));
        assert_eq!(RepoPath::new("/etc/passwd"), Err(RepoPathError::Absolute));
        assert_eq!(
            RepoPath::new("../secret"),
            Err(RepoPathError::ParentTraversal)
        );
        assert_ne!(RepoPathError::Empty.to_string(), "");
        assert_ne!(RepoPathError::Absolute.to_string(), "");
        assert_ne!(RepoPathError::ParentTraversal.to_string(), "");
    }
}
