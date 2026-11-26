//! `garde init` request builder and optional interactive Gum-style wizard launcher.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf_core::{FeatureId, FrameworkId, LanguageId, WorkspaceArchetype};
use gf_engine::{EngineError, InitRequest, SkillsFetchMode, run_init_with_sync_mode};

use crate::args::InitArgs;
use crate::output::{probe_wizard_trigger_env, with_ephemeral_spinner};
use crate::tui::{WizardModel, WizardOutcome, run_wizard_interactive, should_launch_init_wizard};
use crate::ui::{ColorPolicy, Role, paint, render_init_human};

fn resolve_target_dir(opt_path: Option<&PathBuf>, cwd: &Path) -> PathBuf {
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
    let root = resolve_target_dir(args.path.as_ref(), cwd);
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
        if raw.eq_ignore_ascii_case("mern") {
            languages.insert(LanguageId::TypeScript);
            frameworks.extend([
                FrameworkId::NodeJs,
                FrameworkId::Express,
                FrameworkId::Mongoose,
                FrameworkId::React,
            ]);
            continue;
        }
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

pub fn execute_init(
    args: &InitArgs,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    let req = if should_launch_init_wizard(args, probe_wizard_trigger_env()) {
        let root = resolve_target_dir(args.path.as_ref(), cwd);
        let wizard = WizardModel::new(root, args.name.clone(), !args.no_git);
        match run_wizard_interactive(wizard, policy)? {
            WizardOutcome::Confirmed(built) => built,
            WizardOutcome::Cancelled => {
                let msg = paint(
                    "garde: initialisation annulée (aucun fichier écrit).",
                    Role::Muted,
                    policy,
                );
                return Ok((msg, ExitCode::SUCCESS));
            }
        }
    } else {
        build_init_request(args, cwd)?
    };
    let mode = if args.offline {
        SkillsFetchMode::Offline
    } else {
        SkillsFetchMode::Online
    };
    let report = with_ephemeral_spinner(
        !args.offline,
        "Synchronisation des skills et du contexte...",
        policy,
        || run_init_with_sync_mode(&req, mode),
    )?;
    Ok((render_init_human(&report, policy), ExitCode::SUCCESS))
}
