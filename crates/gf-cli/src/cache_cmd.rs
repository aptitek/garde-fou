//! Command handler for `garde cache [status|prune|clean] [PATH]`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf_engine::{EngineError, cache_clean, cache_prune, cache_status};

use crate::args_ops::{CacheArgs, CacheSubcommand};
use crate::tui::detect_term_width;
use crate::ui::{ColorPolicy, render_cache_clean_human, render_cache_status_human};

fn resolve_target(path: Option<&PathBuf>, cwd: &Path) -> PathBuf {
    path.map_or_else(
        || cwd.to_path_buf(),
        |rel| {
            if rel.is_absolute() {
                rel.clone()
            } else {
                cwd.join(rel)
            }
        },
    )
}

pub fn execute_cache(
    args: &CacheArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let root = resolve_target(args.path.as_ref(), cwd);
    let width = detect_term_width();
    match args.action.unwrap_or_default() {
        CacheSubcommand::Status => {
            let report = cache_status(&root)?;
            Ok((
                render_cache_status_human(&report, policy, width),
                ExitCode::SUCCESS,
            ))
        }
        CacheSubcommand::Prune => {
            let summary = cache_prune(&root)?;
            Ok((
                render_cache_clean_human("prune", &summary, policy),
                ExitCode::SUCCESS,
            ))
        }
        CacheSubcommand::Clean => {
            let summary = cache_clean(&root)?;
            Ok((
                render_cache_clean_human("clean", &summary, policy),
                ExitCode::SUCCESS,
            ))
        }
    }
}
