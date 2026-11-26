//! Command handlers for `garde uninstall` and alias `garde eject`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf_engine::{
    EngineError, UninstallExecReport, UninstallPlan, execute_project_eject, execute_self_uninstall,
    plan_project_eject, plan_self_uninstall,
};

use crate::args_ops::{EjectArgs, UninstallArgs, UninstallBehaviorFlags, UninstallScopeFlags};
use crate::output::probe_stdout_capabilities;
use crate::tui::{UninstallChoice, detect_term_width};
use crate::tui_runner::run_uninstall_wizard_interactive;
use crate::ui::{ColorPolicy, render_uninstall_exec_human, render_uninstall_plan_human};

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

#[derive(Clone, Copy)]
struct ResolvedScope {
    project: bool,
    self_bin: bool,
}

fn resolve_scope(
    args: &UninstallArgs,
    policy: ColorPolicy,
) -> Result<Option<ResolvedScope>, EngineError> {
    if args.scope.project || args.scope.self_bin {
        return Ok(Some(ResolvedScope {
            project: args.scope.project,
            self_bin: args.scope.self_bin,
        }));
    }
    let caps = probe_stdout_capabilities();
    if !caps.is_tty || caps.is_ci {
        return Err(EngineError::InvalidConfig(String::from(
            "Spécifiez `--project` et/ou `--self` en mode non-interactif (ou utilisez `garde eject`).",
        )));
    }
    let Some(choice) = run_uninstall_wizard_interactive(policy)? else {
        return Ok(None);
    };
    let resolved = match choice {
        UninstallChoice::Project => ResolvedScope {
            project: true,
            self_bin: false,
        },
        UninstallChoice::SelfBinary => ResolvedScope {
            project: false,
            self_bin: true,
        },
        UninstallChoice::Both => ResolvedScope {
            project: true,
            self_bin: true,
        },
    };
    Ok(Some(resolved))
}

fn build_combined_plan(
    root: &Path,
    scope: ResolvedScope,
    purge_skills: bool,
) -> Result<UninstallPlan, EngineError> {
    let mut plan = UninstallPlan::default();
    if scope.project {
        let proj = plan_project_eject(root, purge_skills)?;
        plan.actions.extend(proj.actions);
        plan.warnings.extend(proj.warnings);
    }
    if scope.self_bin {
        let exe = std::env::current_exe()?;
        let path_env = std::env::var("PATH").ok();
        let self_plan = plan_self_uninstall(&exe, path_env.as_deref());
        plan.actions.extend(self_plan.actions);
        plan.warnings.extend(self_plan.warnings);
    }
    Ok(plan)
}

pub fn execute_uninstall(
    args: &UninstallArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let width = detect_term_width();
    let Some(scope) = resolve_scope(args, policy)? else {
        return Ok((String::from("Désinstallation annulée."), ExitCode::SUCCESS));
    };
    let root = resolve_target(args.path.as_ref(), cwd);
    let plan = build_combined_plan(&root, scope, args.behavior.purge_skills)?;
    if args.behavior.dry_run {
        let rendered = render_uninstall_plan_human(&plan, true, policy, width);
        return Ok((rendered, ExitCode::SUCCESS));
    }
    if !args.yes && (args.scope.project || args.scope.self_bin) {
        let caps = probe_stdout_capabilities();
        if !caps.is_tty || caps.is_ci {
            return Err(EngineError::InvalidConfig(String::from(
                "Confirmation requise en mode non-interactif : ajoutez `-y` / `--yes` ou `--dry-run`.",
            )));
        }
    }
    let mut combined = UninstallExecReport::default();
    if scope.project {
        let rep = execute_project_eject(&root, args.behavior.purge_skills)?;
        combined.applied.extend(rep.applied);
        combined.warnings.extend(rep.warnings);
    }
    if scope.self_bin {
        let exe = std::env::current_exe()?;
        let path_env = std::env::var("PATH").ok();
        let rep = execute_self_uninstall(&exe, path_env.as_deref())?;
        combined.applied.extend(rep.applied);
        combined.warnings.extend(rep.warnings);
    }
    Ok((
        render_uninstall_exec_human(&combined, policy),
        ExitCode::SUCCESS,
    ))
}

pub fn execute_eject(
    args: &EjectArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let synthetic = UninstallArgs {
        path: args.path.clone(),
        scope: UninstallScopeFlags {
            project: true,
            self_bin: false,
        },
        behavior: UninstallBehaviorFlags {
            purge_skills: args.behavior.purge_skills,
            dry_run: args.behavior.dry_run,
        },
        yes: args.yes,
    };
    execute_uninstall(&synthetic, cwd, policy)
}
