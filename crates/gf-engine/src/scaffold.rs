//! Scaffolding support for `garde-fou init`.

use std::fs;
use std::path::{Path, PathBuf};

pub use crate::types::DEFAULT_CONFIG_FILENAME;
use crate::types::EngineError;

const DEFAULT_CONFIG_TEMPLATE: &str = "# Garde-fou configuration
schema_version = 1

[limits]
max_file_lines = 250
max_function_lines = 50
max_parameters = 4
max_cyclomatic_complexity = 10
max_cognitive_complexity = 10
max_nesting_depth = 4
max_imports_per_file = 15
max_methods_per_type = 10
min_clone_lines = 15
min_clone_nodes = 40

[paths]
ignore_paths = []
entrypoints = []

[cache]
enabled = true
directory = \".garde-fou-cache\"
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
