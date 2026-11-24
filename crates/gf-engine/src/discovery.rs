//! Workspace file discovery via `ignore` (`.gitignore`-aware) and `git diff --cached`.

use std::path::Path;
use std::process::Command;

use gf_core::{Config, DomainKind, Language, RepoPath, classify_path, is_audited_tool_config_file};
use gf_parsers::detect_language;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::types::{EngineError, ScanScope};

fn compile_ignore_globs(patterns: &[String]) -> Result<GlobSet, EngineError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| EngineError::InvalidConfig(err.to_string()))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|err| EngineError::InvalidConfig(err.to_string()))
}

fn should_descend(entry_path: &Path, root: &Path) -> bool {
    let Ok(rel) = entry_path.strip_prefix(root) else {
        return true;
    };
    !rel.components().any(|comp| {
        let s = comp.as_os_str();
        s == ".git"
            || s == ".garde-fou-cache"
            || s == "target"
            || s == "node_modules"
            || s == ".agents"
            || s == "mutants.out"
            || s == ".stryker-tmp"
    })
}

fn is_discovered_target(repo_path: &RepoPath, config: &Config) -> bool {
    let lang = detect_language(repo_path);
    lang.is_always_discovered()
        || is_audited_tool_config_file(repo_path)
        || (matches!(lang, Language::Json | Language::Toml)
            && classify_path(repo_path, &config.domains) == DomainKind::Texte)
}

fn discover_all(
    root: &Path,
    config: &Config,
    ignore_set: &GlobSet,
) -> Result<Vec<RepoPath>, EngineError> {
    let root_owned = root.to_path_buf();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(move |entry| should_descend(entry.path(), &root_owned))
        .build();

    let mut paths = Vec::new();
    for item in walker {
        let entry = item.map_err(|err| EngineError::Io(std::io::Error::other(err.to_string())))?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let Some(rel_str) = rel.to_str() else {
            continue;
        };
        let repo_path = RepoPath::new(rel_str)?;
        if is_discovered_target(&repo_path, config) && !ignore_set.is_match(repo_path.as_str()) {
            paths.push(repo_path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn discover_staged(
    root: &Path,
    config: &Config,
    ignore_set: &GlobSet,
) -> Result<Vec<RepoPath>, EngineError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::GitCommand(stderr.trim().to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = Vec::new();
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let repo_path = RepoPath::new(line)?;
        if !root.join(repo_path.as_str()).is_file() {
            continue;
        }
        if is_discovered_target(&repo_path, config) && !ignore_set.is_match(repo_path.as_str()) {
            paths.push(repo_path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Discovers analyzed source files in `root` according to `scope` and `config.ignore_paths`.
///
/// # Errors
///
/// Returns [`EngineError`] if glob compilation, directory traversal, or `git diff` fails.
pub fn discover_files(
    root: &Path,
    scope: ScanScope,
    config: &Config,
) -> Result<Vec<RepoPath>, EngineError> {
    let ignore_set = compile_ignore_globs(&config.ignore_paths)?;
    match scope {
        ScanScope::All => discover_all(root, config, &ignore_set),
        ScanScope::StagedOnly => discover_staged(root, config, &ignore_set),
    }
}
