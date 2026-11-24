//! Semantic `Dockerfile` instruction dispatcher (`FROM`, `SHELL`, `RUN`, `CMD`, `ENTRYPOINT`, `COPY`, etc.).

use gf_core::{
    DockerCommandForm, DockerDocument, DockerInstruction, DockerInstructionKind, DockerStageShell,
    FromStage, LangRuleFinding, Span,
};

use super::dockerfile_args::{
    classify_shell_argv, parse_add, parse_arg, parse_copy, parse_docker_command_form, parse_env,
    parse_non_empty, split_image_ref,
};
use super::dockerfile_run_bridge::{
    ExtractedRunScript, RunBuildEnv, analyze_and_remap_run_script, build_run_script,
};

const OTHER_VALID_KEYWORDS: &[&str] = &[
    "EXPOSE",
    "HEALTHCHECK",
    "LABEL",
    "MAINTAINER",
    "ONBUILD",
    "STOPSIGNAL",
    "VOLUME",
];

/// Mutable parsing state for a `Dockerfile` and its bridged `RUN` scripts.
pub struct InstrParseCtx<'a> {
    /// Target `DockerDocument` being populated.
    pub doc: &'a mut DockerDocument,
    /// Raw lines of the `Dockerfile` source.
    pub source_lines: &'a [&'a str],
    /// Active line-continuation escape character (`\` or `` ` ``).
    pub escape: char,
    /// Output accumulator for bridged Shell/PowerShell `LangRuleFinding`s (`GF080`).
    pub bridged_findings: &'a mut Vec<LangRuleFinding>,
}

fn parse_from(
    args: &str,
    span: Span,
    doc: &mut DockerDocument,
) -> Result<DockerInstructionKind, String> {
    let tokens: Vec<&str> = args
        .split_whitespace()
        .filter(|tok| !tok.starts_with("--"))
        .collect();
    let Some(spec) = tokens.first() else {
        return Err(String::from("FROM instruction requires a base image"));
    };
    let alias = if tokens.len() >= 3 && tokens[1].eq_ignore_ascii_case("AS") {
        Some(String::from(tokens[2]))
    } else if tokens.len() > 1 {
        return Err(String::from(
            "malformed FROM clause (expected `FROM <image> [AS <name>]`)",
        ));
    } else {
        None
    };

    let (image, tag, digest) = split_image_ref(spec);
    let parent_stage = (tag.is_none() && digest.is_none() && !image.contains('/'))
        .then(|| {
            doc.stages.iter().rfind(|s| {
                s.alias
                    .as_deref()
                    .is_some_and(|a| a.eq_ignore_ascii_case(&image))
            })
        })
        .flatten();

    let effective_user = parent_stage.and_then(|s| s.effective_user.clone());
    let effective_shell = parent_stage.map_or_else(DockerStageShell::default_posix, |s| {
        s.effective_shell.clone()
    });

    let stage_index = doc.stages.len();
    doc.stages.push(FromStage {
        image,
        tag,
        digest,
        alias,
        effective_user,
        effective_shell,
        span,
    });
    Ok(DockerInstructionKind::From { stage_index })
}

fn parse_shell_instr(
    rest: &str,
    doc: &mut DockerDocument,
) -> Result<DockerInstructionKind, String> {
    let raw = parse_non_empty("SHELL", rest)?;
    let form = parse_docker_command_form(&raw);
    let shell = match &form {
        DockerCommandForm::ExecForm(argv) if !argv.is_empty() => classify_shell_argv(argv),
        _ => {
            return Err(String::from(
                "SHELL instruction requires a non-empty JSON string array",
            ));
        }
    };
    if let Some(stage) = doc.stages.last_mut() {
        stage.effective_shell = shell.clone();
    }
    Ok(DockerInstructionKind::Shell { form, shell })
}

fn dispatch_run(
    rest: &str,
    span: Span,
    ctx: &mut InstrParseCtx<'_>,
) -> Result<DockerInstructionKind, String> {
    let args = parse_non_empty("RUN", rest)?;
    let shell_kind = ctx.doc.active_shell_kind();
    let env = RunBuildEnv {
        source_lines: ctx.source_lines,
        escape: ctx.escape,
        shell_kind,
    };
    let extracted: ExtractedRunScript = build_run_script(&env, span, &args);
    let has_unguarded_pipe =
        analyze_and_remap_run_script(&extracted, shell_kind, span, ctx.bridged_findings)
            .map_err(|e| e.to_string())?;
    Ok(DockerInstructionKind::Run {
        command: extracted.collapsed_command,
        form: extracted.form,
        has_unguarded_pipe,
    })
}

/// Parses a single logical `Dockerfile` instruction line and appends it to `ctx.doc`.
pub fn parse_instruction(raw: &str, span: Span, ctx: &mut InstrParseCtx<'_>) -> Result<(), String> {
    let trimmed = raw.trim();
    let (kw_raw, rest) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(k, r)| (k, r.trim()));
    let keyword = kw_raw.to_ascii_uppercase();

    if ctx.doc.stages.is_empty() && keyword != "FROM" && keyword != "ARG" {
        return Err(format!(
            "instruction `{keyword}` cannot appear before the first `FROM` stage"
        ));
    }

    let kind = dispatch_keyword(&keyword, rest, span, ctx)?;
    ctx.doc.instructions.push(DockerInstruction { kind, span });
    Ok(())
}

fn dispatch_data_or_other(
    keyword: &str,
    rest: &str,
    doc: &DockerDocument,
) -> Result<DockerInstructionKind, String> {
    match keyword {
        "COPY" => parse_copy(rest, doc),
        "ADD" => parse_add(rest),
        "WORKDIR" => Ok(DockerInstructionKind::Workdir {
            path: parse_non_empty("WORKDIR", rest)?,
        }),
        "ENV" => parse_env(rest),
        "ARG" => parse_arg(rest),
        other if OTHER_VALID_KEYWORDS.contains(&other) => Ok(DockerInstructionKind::Other {
            keyword: String::from(other),
            arguments: String::from(rest),
        }),
        other => Err(format!("unknown Dockerfile instruction `{other}`")),
    }
}

fn dispatch_keyword(
    keyword: &str,
    rest: &str,
    span: Span,
    ctx: &mut InstrParseCtx<'_>,
) -> Result<DockerInstructionKind, String> {
    match keyword {
        "FROM" => parse_from(rest, span, ctx.doc),
        "USER" => {
            let user = parse_non_empty("USER", rest)?;
            if let Some(stage) = ctx.doc.stages.last_mut() {
                stage.effective_user = Some(user.clone());
            }
            Ok(DockerInstructionKind::User { user })
        }
        "SHELL" => parse_shell_instr(rest, ctx.doc),
        "RUN" => dispatch_run(rest, span, ctx),
        "CMD" => {
            let arguments = parse_non_empty("CMD", rest)?;
            let form = parse_docker_command_form(&arguments);
            Ok(DockerInstructionKind::Cmd { arguments, form })
        }
        "ENTRYPOINT" => {
            let arguments = parse_non_empty("ENTRYPOINT", rest)?;
            let form = parse_docker_command_form(&arguments);
            Ok(DockerInstructionKind::Entrypoint { arguments, form })
        }
        other => dispatch_data_or_other(other, rest, ctx.doc),
    }
}
