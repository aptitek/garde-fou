//! Direct `gf-rules` unit tests for `DK-DL4006` (`SHELL` inheritance), `DK-DL3025` (`CMD`/`ENTRYPOINT`
//! exec vs shell form), and `DK-DL3022` (`COPY --from` stage/index/external validation).

use std::collections::BTreeSet;

use gf_core::{
    Config, DockerCommandForm, DockerDocument, DockerInstruction, DockerInstructionKind,
    DockerShellKind, DockerStageShell, FeatureId, FromStage, Language, ProjectContext, RepoPath,
    Span, UastDocument,
};
use gf_rules::evaluate_workspace_sorted;

type Res = Result<(), Box<dyn std::error::Error>>;

fn make_ctx() -> ProjectContext {
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Docker);
    ctx.dockerignore_entries = Some(BTreeSet::from([String::from(".git")]));
    ctx
}

fn mk_stage(
    image: &str,
    tag: Option<&str>,
    digest: Option<&str>,
    alias: Option<&str>,
) -> FromStage {
    FromStage {
        image: String::from(image),
        tag: tag.map(String::from),
        digest: digest.map(String::from),
        alias: alias.map(String::from),
        effective_user: Some(String::from("appuser")),
        effective_shell: DockerStageShell::default_posix(),
        span: Span::FIRST,
    }
}

const fn mk_instr(kind: DockerInstructionKind) -> DockerInstruction {
    DockerInstruction {
        kind,
        span: Span::FIRST,
    }
}

fn eval_doc(doc: DockerDocument) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut uast = UastDocument::new(RepoPath::new("Dockerfile")?, Language::Dockerfile, 12, 10)?;
    uast.docker = Some(doc);
    Ok(
        evaluate_workspace_sorted(&[uast], &make_ctx(), &Config::default())
            .into_iter()
            .map(|d| format!("{}:{}", d.rule_id, d.message))
            .collect(),
    )
}

fn mk_copy(from: &str, stage_index: usize) -> DockerInstruction {
    mk_instr(DockerInstructionKind::Copy {
        arguments: format!("--from={from} /a /b"),
        copy_from: Some(from.into()),
        stage_index,
    })
}

fn bash_pipefail_shell() -> DockerStageShell {
    DockerStageShell {
        kind: DockerShellKind::Bash,
        argv: vec![
            "/bin/bash".into(),
            "-o".into(),
            "pipefail".into(),
            "-c".into(),
        ],
        pipefail_enabled: true,
    }
}

#[test]
fn dl4006_shell_inheritance_and_exec_form_allow_valid_pipes() -> Res {
    let bash_pf = bash_pipefail_shell();
    let inherited_ok = DockerDocument {
        stages: vec![
            mk_stage("ubuntu", Some("24.04"), None, Some("base")),
            mk_stage("base", None, None, Some("child")),
        ],
        instructions: vec![
            mk_instr(DockerInstructionKind::From { stage_index: 0 }),
            mk_instr(DockerInstructionKind::Shell {
                form: DockerCommandForm::ExecForm(bash_pf.argv.clone()),
                shell: bash_pf,
            }),
            mk_instr(DockerInstructionKind::Run {
                command: "curl -s https://x | sh".into(),
                form: DockerCommandForm::ShellForm,
                has_unguarded_pipe: true,
            }),
            mk_instr(DockerInstructionKind::From { stage_index: 1 }),
            mk_instr(DockerInstructionKind::Run {
                command: "curl -s https://y | sh".into(),
                form: DockerCommandForm::ShellForm,
                has_unguarded_pipe: true,
            }),
            mk_instr(DockerInstructionKind::Run {
                command: "[\"sh\", \"-c\", \"a | b\"]".into(),
                form: DockerCommandForm::ExecForm(vec!["sh".into(), "-c".into()]),
                has_unguarded_pipe: true,
            }),
        ],
    };
    assert_eq!(eval_doc(inherited_ok)?, Vec::<String>::new());
    Ok(())
}

#[test]
fn dl4006_non_alias_stages_and_shell_resets_report_missing_pipefail() -> Res {
    let bash_pf = bash_pipefail_shell();
    let posix_sh = DockerStageShell::default_posix();
    for non_alias_stage in [
        mk_stage("base", Some("24.04"), None, None),
        mk_stage("base", None, Some("sha256:1234"), None),
        mk_stage("org/base", Some("1.0"), None, None),
    ] {
        let doc = DockerDocument {
            stages: vec![
                mk_stage("ubuntu", Some("24.04"), None, Some("base")),
                non_alias_stage,
            ],
            instructions: vec![
                mk_instr(DockerInstructionKind::From { stage_index: 0 }),
                mk_instr(DockerInstructionKind::Shell {
                    form: DockerCommandForm::ExecForm(bash_pf.argv.clone()),
                    shell: bash_pf.clone(),
                }),
                mk_instr(DockerInstructionKind::From { stage_index: 1 }),
                mk_instr(DockerInstructionKind::Run {
                    command: "curl -s https://z | sh".into(),
                    form: DockerCommandForm::ShellForm,
                    has_unguarded_pipe: true,
                }),
            ],
        };
        let msgs = eval_doc(doc)?;
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("DK-DL4006"));
    }
    let reset_doc = DockerDocument {
        stages: vec![mk_stage("ubuntu", Some("24.04"), None, Some("base"))],
        instructions: vec![
            mk_instr(DockerInstructionKind::From { stage_index: 99 }),
            mk_instr(DockerInstructionKind::Shell {
                form: DockerCommandForm::ExecForm(posix_sh.argv.clone()),
                shell: posix_sh,
            }),
            mk_instr(DockerInstructionKind::Run {
                command: "a | b".into(),
                form: DockerCommandForm::ShellForm,
                has_unguarded_pipe: true,
            }),
        ],
    };
    assert_eq!(eval_doc(reset_doc)?.len(), 1);
    Ok(())
}

#[test]
fn dl3025_and_dl3022_copy_from_exact_boundaries() -> Res {
    let stages = vec![
        mk_stage("rust", Some("1.88"), None, Some("builder")),
        mk_stage("debian", Some("12.8-slim"), None, Some("runtime")),
    ];
    let valid_doc = DockerDocument {
        stages,
        instructions: vec![
            mk_instr(DockerInstructionKind::From { stage_index: 0 }),
            mk_instr(DockerInstructionKind::From { stage_index: 1 }),
            mk_copy("BUILDER", 1),
            mk_copy("0", 1),
            mk_copy("$IMG", 1),
            mk_copy("alpine:3.20", 1),
            mk_copy("alpine@sha256:11", 1),
            mk_copy("ghcr.io/x", 1),
            mk_instr(DockerInstructionKind::Entrypoint {
                arguments: "[\"/bin/app\"]".into(),
                form: DockerCommandForm::ExecForm(vec!["/bin/app".into()]),
            }),
            mk_instr(DockerInstructionKind::Cmd {
                arguments: "[\"--serve\"]".into(),
                form: DockerCommandForm::ExecForm(vec!["--serve".into()]),
            }),
        ],
    };
    assert_eq!(eval_doc(valid_doc)?, Vec::<String>::new());
    Ok(())
}

#[test]
fn dl3025_and_dl3022_reject_invalid_copy_from_and_shell_form_cmd() -> Res {
    let stages = vec![
        mk_stage("rust", Some("1.88"), None, Some("builder")),
        mk_stage("debian", Some("12.8-slim"), None, Some("runtime")),
    ];
    let invalid_doc = DockerDocument {
        stages,
        instructions: vec![
            mk_instr(DockerInstructionKind::From { stage_index: 0 }),
            mk_instr(DockerInstructionKind::From { stage_index: 1 }),
            mk_copy("1", 1),
            mk_copy("2", 1),
            mk_copy("\"\"", 1),
            mk_copy("missing", 1),
            mk_instr(DockerInstructionKind::Entrypoint {
                arguments: "/bin/app".into(),
                form: DockerCommandForm::ShellForm,
            }),
            mk_instr(DockerInstructionKind::Cmd {
                arguments: "/bin/app --serve".into(),
                form: DockerCommandForm::ShellForm,
            }),
        ],
    };
    let errs = eval_doc(invalid_doc)?;
    assert_eq!(errs.iter().filter(|m| m.contains("DK-DL3022")).count(), 4);
    assert_eq!(errs.iter().filter(|m| m.contains("DK-DL3025")).count(), 2);
    Ok(())
}
