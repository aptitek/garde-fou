//! Subcommand execution handlers for `garde` (`check`, `tui`, `config`, `sync`, `lock`, `commit-msg`).

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use gf_core::validate_commit_message;
use gf_engine::{
    CheckOptions, EngineError, ScanScope, SkillsFetchMode, SyncOptions, load_workspace_config,
    run_check_with_options, run_lock, run_sync,
};

use crate::args::{
    CheckArgs, CheckExecutionFlags, CommitMsgArgs, LockArgs, OutputFormat, SyncArgs, TuiArgs,
};
use crate::args_ops::ConfigArgs;
use crate::output::with_ephemeral_spinner;
use crate::reporter::render_check_report_rich;
use crate::source_loader::{load_diagnosed_sources, read_lcov_tenths};
use crate::tui::{AppTab, detect_term_width};
use crate::tui_runner::{AppRunParams, run_app_interactive};
use crate::ui::{
    ColorPolicy, HumanCheckMeta, render_commit_msg_human, render_lock_human, render_sync_human,
};
use crate::watch_runner::run_watch_check_loop;

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

const fn check_options_from(
    exec: CheckExecutionFlags,
    strict_tools: bool,
    lcov: Option<String>,
    mutants: Option<String>,
) -> CheckOptions {
    CheckOptions {
        scope: if exec.staged {
            ScanScope::StagedOnly
        } else {
            ScanScope::All
        },
        no_cache: exec.no_cache,
        strict_tools,
        lcov_path: lcov,
        mutants_path: mutants,
    }
}

pub fn execute_check(
    args: &CheckArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    if args.mode.interactive {
        let tui = TuiArgs {
            exec: args.exec,
            strict_tools: args.strict_tools,
            lcov: args.lcov.clone(),
            mutants: args.mutants.clone(),
        };
        return execute_tui(&tui, cwd, policy);
    }
    let options = check_options_from(
        args.exec,
        args.strict_tools,
        args.lcov.clone(),
        args.mutants.clone(),
    );
    if args.mode.watch {
        let code = run_watch_check_loop(cwd, &options, policy)?;
        return Ok((String::new(), code));
    }
    let t0 = Instant::now();
    let report = run_check_with_options(cwd, &options)?;
    let elapsed_ms = t0.elapsed().as_millis();
    let exit = if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    let sources = if args.format == OutputFormat::Human {
        load_diagnosed_sources(cwd, &report)
    } else {
        BTreeMap::new()
    };
    let meta = HumanCheckMeta {
        elapsed_ms,
        lcov_tenths: (args.format == OutputFormat::Human)
            .then(|| read_lcov_tenths(cwd, args.lcov.as_deref()))
            .flatten(),
        policy,
        max_width: detect_term_width(),
    };
    Ok((
        render_check_report_rich(&report, args.format, &sources, &meta),
        exit,
    ))
}

pub fn execute_tui(
    args: &TuiArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let initial_opts = check_options_from(
        args.exec,
        args.strict_tools,
        args.lcov.clone(),
        args.mutants.clone(),
    );
    let params = AppRunParams {
        initial_tab: AppTab::Diagnostics,
        initial_opts: &initial_opts,
        fetch_mode: SkillsFetchMode::Offline,
        policy,
    };
    let code = run_app_interactive(cwd, &params)?;
    Ok((String::new(), code))
}

pub fn execute_config(
    args: &ConfigArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let root = resolve_dir(args.path.as_ref(), cwd);
    let opts = CheckOptions::default();
    let fetch_mode = if args.offline {
        SkillsFetchMode::Offline
    } else {
        SkillsFetchMode::Online
    };
    let params = AppRunParams {
        initial_tab: AppTab::Config,
        initial_opts: &opts,
        fetch_mode,
        policy,
    };
    let code = run_app_interactive(&root, &params)?;
    Ok((String::new(), code))
}

pub fn execute_sync(
    args: &SyncArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let target_root = resolve_dir(args.path.as_ref(), cwd);
    let fetch_mode = if args.offline {
        SkillsFetchMode::Offline
    } else if let Some(dir) = &args.skills_from_dir {
        SkillsFetchMode::FromDir(if dir.is_absolute() {
            dir.clone()
        } else {
            cwd.join(dir)
        })
    } else {
        SkillsFetchMode::Online
    };
    let is_online = fetch_mode == SkillsFetchMode::Online;
    let options = SyncOptions {
        fetch_mode,
        update_skills: args.update_skills,
    };
    let report =
        with_ephemeral_spinner(is_online, "Synchronisation des skills...", policy, || {
            run_sync(&target_root, &options)
        })?;
    Ok((render_sync_human(&report, policy), ExitCode::SUCCESS))
}

pub fn execute_lock(
    args: &LockArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let target_root = resolve_dir(args.path.as_ref(), cwd);
    let digest = run_lock(&target_root)?;
    Ok((render_lock_human(&digest, policy), ExitCode::SUCCESS))
}

pub fn execute_commit_msg(
    args: &CommitMsgArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let msg_path = if args.msg_file.is_absolute() {
        args.msg_file.clone()
    } else {
        cwd.join(&args.msg_file)
    };
    let loaded = load_workspace_config(cwd)?;
    let raw_msg = fs::read_to_string(&msg_path)?;
    let outcome = validate_commit_message(&raw_msg, &loaded.analyzer.commit);
    let exit = if outcome.is_valid() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    let rendered = render_commit_msg_human(&outcome, policy, detect_term_width());
    Ok((rendered, exit))
}
