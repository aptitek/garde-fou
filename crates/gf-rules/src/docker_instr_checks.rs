//! Instruction-level `Dockerfile` security (`GF032`), layer hygiene (`GF033`),
//! and `Hadolint`/`BuildKit` rules (`DK-DL4006`, `DK-DL3025`, `DK-DL3022`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Diagnostic, DockerCommandForm, DockerInstruction, DockerInstructionKind, FromStage, RepoPath,
};

use crate::checks::{SourceLoc, rule_error as docker_error};

const SECRET_SUBSTRINGS: &[&str] = &["SECRET", "PASSWORD", "TOKEN", "PRIVATE_KEY", "API_KEY"];
const ARCHIVE_SUFFIXES: &[&str] = &[".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tar.xz", ".tar.zst"];

/// Evaluation context for a single [`DockerInstruction`].
pub struct DockerInstrCtx<'a> {
    /// `Dockerfile` path.
    pub path: &'a RepoPath,
    /// All parsed stages in the `Dockerfile`.
    pub stages: &'a [FromStage],
    /// Whether the active stage's `SHELL` already guarantees `pipefail` (`DK-DL4006`).
    pub active_pipefail: bool,
}

fn is_hardcoded_secret(key: &str, value: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    let sensitive_name = SECRET_SUBSTRINGS
        .iter()
        .any(|needle| upper.contains(needle));
    let trimmed = value.trim();
    sensitive_name && !trimmed.is_empty() && !trimmed.starts_with('$')
}

fn is_justified_add_source(src: &str) -> bool {
    let lower = src.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || ARCHIVE_SUFFIXES.iter().any(|ext| lower.ends_with(ext))
}

struct RunInstrInfo<'a> {
    command: &'a str,
    form: &'a DockerCommandForm,
    has_unguarded_pipe: bool,
    active_pipefail: bool,
}

fn check_run_pkg_caches(command: &str, loc: SourceLoc<'_>, out: &mut Vec<Diagnostic>) {
    if command.contains("apt-get install")
        && !command.contains("/var/lib/apt/lists")
        && !command.contains("--mount=type=cache")
    {
        out.push(docker_error(
            "GF033",
            String::from("`apt-get install` leaves package lists in image layer"),
            "Clean `/var/lib/apt/lists/*` in the same `RUN` layer or use `RUN --mount=type=cache,target=/var/cache/apt`.",
            loc,
        ));
    }
    if command.contains("pip install")
        && !command.contains("--no-cache-dir")
        && !command.contains("--mount=type=cache")
    {
        out.push(docker_error(
            "GF033",
            String::from("`pip install` retains wheel cache inside image layer"),
            "Pass `--no-cache-dir` to `pip install` or mount a BuildKit cache directory.",
            loc,
        ));
    }
}

fn check_run_command(loc: SourceLoc<'_>, run: &RunInstrInfo<'_>, out: &mut Vec<Diagnostic>) {
    let uses_sudo = run
        .command
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .any(|tok| tok == "sudo");
    if uses_sudo {
        out.push(docker_error(
            "GF032",
            String::from("`RUN` invokes `sudo` inside container build"),
            "Execute root build steps before switching `USER`, or use `gosu`/`su-exec` instead of `sudo`.",
            loc,
        ));
    }
    check_run_pkg_caches(run.command, loc, out);
    if *run.form == DockerCommandForm::ShellForm && run.has_unguarded_pipe && !run.active_pipefail {
        out.push(docker_error(
            "GF033",
            String::from(
                "[DK-DL4006] `RUN` contains a pipeline `|` without `-o pipefail` in `SHELL` or `set -o pipefail`",
            ),
            "Set `SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"]` before `RUN` pipelines or prepend `set -o pipefail;` inside `RUN`.",
            loc,
        ));
    }
}

fn check_env_secrets(bindings: &[(String, String)], loc: SourceLoc<'_>, out: &mut Vec<Diagnostic>) {
    for (k, v) in bindings {
        if is_hardcoded_secret(k, v) {
            out.push(docker_error(
                "GF032",
                format!("hardcoded secret value in `ENV {k}`"),
                "Mount secrets at build time via `RUN --mount=type=secret` or inject them at runtime.",
                loc,
            ));
        }
    }
}

fn is_valid_copy_from_ref(raw_ref: &str, stage_index: usize, stages: &[FromStage]) -> bool {
    let clean = raw_ref.trim().trim_matches(['"', '\'']);
    if clean.is_empty() {
        return false;
    }
    if clean.contains('$') || clean.contains(':') || clean.contains('@') || clean.contains('/') {
        return true;
    }
    if stages.iter().take(stage_index).any(|s| {
        s.alias
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case(clean))
    }) {
        return true;
    }
    clean.parse::<usize>().is_ok_and(|idx| idx < stage_index)
}

fn check_shell_form_entrypoint_or_cmd(
    kw: &str,
    form: &DockerCommandForm,
    loc: SourceLoc<'_>,
    out: &mut Vec<Diagnostic>,
) {
    if *form == DockerCommandForm::ShellForm {
        out.push(docker_error(
            "GF033",
            format!("[DK-DL3025] `{kw}` uses shell form instead of JSON exec array `[\"app\", ...]`"),
            "Use JSON exec notation (e.g. `CMD [\"app\", \"--serve\"]`) so PID 1 receives OS signals (`SIGTERM`) directly without `/bin/sh -c`.",
            loc,
        ));
    }
}

fn check_add_operands(operands: &[String], loc: SourceLoc<'_>, out: &mut Vec<Diagnostic>) {
    let sources = &operands[..operands.len().saturating_sub(1)];
    if sources.iter().any(|s| !is_justified_add_source(s)) {
        out.push(docker_error(
            "GF032",
            String::from("`ADD` used for local non-archive copy instead of `COPY`"),
            "Replace `ADD` with explicit `COPY` unless extracting a local tar archive.",
            loc,
        ));
    }
}

fn check_copy_from(
    from_ref: &str,
    stage_index: usize,
    stages: &[FromStage],
    loc: SourceLoc<'_>,
) -> Option<Diagnostic> {
    (!is_valid_copy_from_ref(from_ref, stage_index, stages)).then(|| {
        docker_error(
            "GF032",
            format!(
                "[DK-DL3022] `COPY --from={from_ref}` does not match a previously defined `FROM ... AS` alias, prior stage index, or external image"
            ),
            "Reference a prior stage alias declared via `FROM <image> AS <alias>` or an explicit `repo:tag` / `@sha256:...` image.",
            loc,
        )
    })
}

fn check_arg_or_workdir(
    kind: &DockerInstructionKind,
    loc: SourceLoc<'_>,
    out: &mut Vec<Diagnostic>,
) {
    match kind {
        DockerInstructionKind::Arg {
            name,
            default_value: Some(val),
        } if is_hardcoded_secret(name, val) => {
            out.push(docker_error(
                "GF032",
                format!("hardcoded secret default in `ARG {name}`"),
                "Never bake secret defaults into `ARG` (stored in image history); use `RUN --mount=type=secret`.",
                loc,
            ));
        }
        DockerInstructionKind::Workdir { path: wd }
            if !wd.starts_with('/') && !wd.starts_with('$') =>
        {
            out.push(docker_error(
                "GF033",
                format!("relative `WORKDIR {wd}` is fragile across base images"),
                "Always specify an absolute path in `WORKDIR` (e.g. `WORKDIR /app`).",
                loc,
            ));
        }
        _ => {}
    }
}

/// Evaluates `GF032` and `GF033` (including `DK-DL4006`, `DK-DL3025`, `DK-DL3022`) on a [`DockerInstruction`].
pub fn check_instruction(
    ctx: &DockerInstrCtx<'_>,
    instr: &DockerInstruction,
    out: &mut Vec<Diagnostic>,
) {
    let loc = SourceLoc::new(ctx.path, instr.span);
    match &instr.kind {
        DockerInstructionKind::Add { operands } => check_add_operands(operands, loc, out),
        DockerInstructionKind::Run {
            command,
            form,
            has_unguarded_pipe,
        } => {
            let run = RunInstrInfo {
                command,
                form,
                has_unguarded_pipe: *has_unguarded_pipe,
                active_pipefail: ctx.active_pipefail,
            };
            check_run_command(loc, &run, out);
        }
        DockerInstructionKind::Cmd { form, .. } => {
            check_shell_form_entrypoint_or_cmd("CMD", form, loc, out);
        }
        DockerInstructionKind::Entrypoint { form, .. } => {
            check_shell_form_entrypoint_or_cmd("ENTRYPOINT", form, loc, out);
        }
        DockerInstructionKind::Copy {
            copy_from: Some(from_ref),
            stage_index,
            ..
        } => out.extend(check_copy_from(from_ref, *stage_index, ctx.stages, loc)),
        DockerInstructionKind::Env { bindings } => check_env_secrets(bindings, loc, out),
        other => check_arg_or_workdir(other, loc, out),
    }
}
