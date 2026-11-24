//! Fast `./node_modules/.bin/<tool>` -> `PATH` resolver for TypeScript/React verifiers (`GF082`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn probe_node_modules_bin(root: &Path, tool_name: &str) -> Option<PathBuf> {
    let bin_dir = root.join("node_modules").join(".bin");
    let unix_bin = bin_dir.join(tool_name);
    if unix_bin.is_file() {
        return Some(unix_bin);
    }
    for ext in ["cmd", "exe"] {
        let win_bin = bin_dir.join(format!("{tool_name}.{ext}"));
        if win_bin.is_file() {
            return Some(win_bin);
        }
    }
    None
}

/// Resolves a Node/TypeScript CLI binary (`oxlint`, `biome`, `tsc`) in `./node_modules/.bin/`
/// first, falling back to `PATH` without invoking `sh -c`.
#[must_use]
pub fn resolve_node_tool(root: &Path, tool_name: &str) -> Option<PathBuf> {
    if let Some(local_bin) = probe_node_modules_bin(root, tool_name) {
        return Some(local_bin);
    }
    let in_path = Command::new(tool_name)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    in_path.then(|| PathBuf::from(tool_name))
}
