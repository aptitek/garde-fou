//! Subcommand execution handlers for `garde` (`check`, `init`, `lock`, `commit-msg`).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf_core::{
    FeatureId, FrameworkId, LanguageId, Validation, WorkspaceArchetype, validate_commit_message,
};
use gf_engine::{
    CheckOptions, EngineError, InitRequest, ScanScope, SkillsFetchMode, SyncOptions,
    load_workspace_config, run_check_with_options, run_init_with_sync_mode, run_lock, run_sync,
};

use crate::args::{CheckArgs, CommitMsgArgs, InitArgs, LockArgs, SyncArgs};
use crate::reporter::{render_check_report, render_init_report, render_sync_report};

fn resolve_dir(opt_path: Option<&PathBuf>, cwd: &Path) -> PathBuf {
    opt_path.map_or_else(
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

fn build_init_request(args: &InitArgs, cwd: &Path) -> Result<InitRequest, EngineError> {
    let root = resolve_dir(args.path.as_ref(), cwd);
    let archetype = args
        .archetype
        .as_deref()
        .map(|raw| {
            WorkspaceArchetype::from_config_str(raw)
                .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --archetype `{raw}`")))
        })
        .transpose()?;
    let mut languages = BTreeSet::new();
    for raw in &args.langs {
        let id = LanguageId::from_slug(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --lang `{raw}`")))?;
        languages.insert(id);
    }
    let mut features = BTreeSet::new();
    for raw in &args.features {
        let id = FeatureId::from_config_str(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --feature `{raw}`")))?;
        features.insert(id);
    }
    let mut frameworks = BTreeSet::new();
    for raw in &args.frameworks {
        let id = FrameworkId::from_config_str(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --framework `{raw}`")))?;
        frameworks.insert(id);
    }

    Ok(InitRequest {
        root,
        project_name: args.name.clone(),
        archetype,
        languages,
        features,
        frameworks,
        run_git_init: !args.no_git,
    })
}

pub fn execute_check(args: &CheckArgs, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    let scope = if args.exec.staged {
        ScanScope::StagedOnly
    } else {
        ScanScope::All
    };
    let options = CheckOptions {
        scope,
        no_cache: args.exec.no_cache,
        strict_tools: args.strict_tools,
        lcov_path: args.lcov.clone(),
        mutants_path: args.mutants.clone(),
    };
    let report = run_check_with_options(cwd, &options)?;
    let exit = if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    Ok((render_check_report(&report, args.format), exit))
}

pub fn execute_init(args: &InitArgs, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    let req = build_init_request(args, cwd)?;
    let mode = if args.offline {
        SkillsFetchMode::Offline
    } else {
        SkillsFetchMode::Online
    };
    let report = run_init_with_sync_mode(&req, mode)?;
    Ok((render_init_report(&report), ExitCode::SUCCESS))
}

pub fn execute_sync(args: &SyncArgs, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    let target_root = resolve_dir(args.path.as_ref(), cwd);
    let fetch_mode = if args.offline {
        SkillsFetchMode::Offline
    } else if let Some(dir) = &args.skills_from_dir {
        let resolved = if dir.is_absolute() {
            dir.clone()
        } else {
            cwd.join(dir)
        };
        SkillsFetchMode::FromDir(resolved)
    } else {
        SkillsFetchMode::Online
    };
    let options = SyncOptions {
        fetch_mode,
        update_skills: args.update_skills,
    };
    let report = run_sync(&target_root, &options)?;
    Ok((render_sync_report(&report), ExitCode::SUCCESS))
}

pub fn execute_lock(args: &LockArgs, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    let target_root = resolve_dir(args.path.as_ref(), cwd);
    let digest = run_lock(&target_root)?;
    let short = digest.get(0..16).unwrap_or(&digest);
    Ok((
        format!("garde: locked `garde-fou.toml` -> `garde.lock` (blake3={short})"),
        ExitCode::SUCCESS,
    ))
}

pub fn execute_commit_msg(
    args: &CommitMsgArgs,
    cwd: &Path,
) -> Result<(String, ExitCode), EngineError> {
    let msg_path = if args.msg_file.is_absolute() {
        args.msg_file.clone()
    } else {
        cwd.join(&args.msg_file)
    };
    let loaded = load_workspace_config(cwd)?;
    let raw_msg = fs::read_to_string(&msg_path)?;
    match validate_commit_message(&raw_msg, &loaded.analyzer.commit) {
        Validation::Valid(()) => Ok((String::from("garde: commit-msg ok"), ExitCode::SUCCESS)),
        Validation::Invalid(diags) => {
            let mut out = String::from("garde: commit-msg FAIL");
            for d in &diags {
                let _ = write!(
                    out,
                    "\n[{}] {}: {}\n  help: {}",
                    d.rule_id, d.path, d.message, d.remediation
                );
            }
            Ok((out, ExitCode::FAILURE))
        }
    }
}
