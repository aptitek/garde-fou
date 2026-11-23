//! Scaffolding support for `garde-fou init`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::EngineError;

/// Canonical configuration filename at the root of a repository.
pub const DEFAULT_CONFIG_FILENAME: &str = "garde-fou.toml";

const DEFAULT_CONFIG_TEMPLATE: &str = "# Garde-fou configuration
schema_version = 1

[limits]
max_file_lines = 250
max_function_lines = 50
max_cognitive_complexity = 10
max_function_params = 4
max_struct_bools = 2
";

/// Outcome of running [`scaffold_config`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitOutcome {
    /// Created a brand-new `garde-fou.toml` file.
    Created(PathBuf),
    /// Preserved an existing `garde-fou.toml` file without overwriting it.
    AlreadyPresent(PathBuf),
}

/// Creates a default `garde-fou.toml` inside `root` if none exists yet.
///
/// # Errors
///
/// Returns [`EngineError::Io`] if creating directories or writing the file fails.
pub fn scaffold_config(root: &Path) -> Result<InitOutcome, EngineError> {
    let target = root.join(DEFAULT_CONFIG_FILENAME);
    if target.exists() {
        return Ok(InitOutcome::AlreadyPresent(target));
    }
    fs::create_dir_all(root)?;
    fs::write(&target, DEFAULT_CONFIG_TEMPLATE)?;
    Ok(InitOutcome::Created(target))
}
