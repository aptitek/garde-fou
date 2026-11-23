//! Instruction-level `Dockerfile` security (`GF032`) and layer hygiene (`GF033`) rules.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{Diagnostic, DockerInstruction, DockerInstructionKind, RepoPath, Span};

use crate::docker_checks::docker_error;

const SECRET_SUBSTRINGS: &[&str] = &["SECRET", "PASSWORD", "TOKEN", "PRIVATE_KEY", "API_KEY"];
const ARCHIVE_SUFFIXES: &[&str] = &[".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tar.xz", ".tar.zst"];

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

fn check_run_command(path: &RepoPath, command: &str, span: Span, out: &mut Vec<Diagnostic>) {
    let loc = (path, span);
    let uses_sudo = command
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

fn check_env_secrets(
    bindings: &[(String, String)],
    loc: (&RepoPath, Span),
    out: &mut Vec<Diagnostic>,
) {
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

/// Evaluates `GF032` and `GF033` on a single parsed [`DockerInstruction`].
pub fn check_instruction(path: &RepoPath, instr: &DockerInstruction, out: &mut Vec<Diagnostic>) {
    let loc = (path, instr.span);
    match &instr.kind {
        DockerInstructionKind::Add { operands } => {
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
        DockerInstructionKind::Run { command } => check_run_command(path, command, instr.span, out),
        DockerInstructionKind::Env { bindings } => check_env_secrets(bindings, loc, out),
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
