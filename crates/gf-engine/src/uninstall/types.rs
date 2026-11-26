//! Leaf types (`UninstallAction`, `UninstallPlan`, `UninstallExecReport`) for `garde uninstall`.

use std::path::PathBuf;

/// Single planned or applied action during `garde uninstall`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UninstallAction {
    /// Remove a Garde-fou-owned file completely.
    DeleteFile(String),
    /// Remove a Garde-fou-owned directory recursively.
    DeleteDir(String),
    /// Surgically strip only the Garde-fou managed block while preserving human content.
    StripManagedBlock(String),
    /// Explicitly preserve a user-authored custom artifact (e.g. custom skill).
    PreserveCustom(String),
    /// Uninstall a `mise`-managed tool (`mise uninstall <slug>` + `mise reshim`).
    MiseUninstall(String),
    /// Remove the standalone `garde` binary at the given path.
    RemoveSelfBinary(PathBuf),
    /// Informational notice about another `garde` executable still present on `PATH`.
    NoteSecondaryBinary(PathBuf),
}

/// Pure, inspectable uninstall plan supporting `--dry-run` and interactive confirmation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallPlan {
    /// Ordered actions to execute (or preview).
    pub actions: Vec<UninstallAction>,
    /// Non-fatal warnings surfaced to the user.
    pub warnings: Vec<String>,
}

impl UninstallPlan {
    /// Returns `true` when zero mutating actions are needed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// Execution summary returned after applying an [`UninstallPlan`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallExecReport {
    /// Actions that were actually applied.
    pub applied: Vec<UninstallAction>,
    /// Non-fatal warnings (e.g. secondary `garde` binaries still on `PATH`).
    pub warnings: Vec<String>,
}
