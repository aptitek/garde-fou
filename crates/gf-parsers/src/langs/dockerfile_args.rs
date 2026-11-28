//! Pure `Dockerfile` operand parsers (`ENV`, `ARG`, `COPY`, `ADD`, `SHELL`, image refs).

use gf_core::{
    DockerCommandForm, DockerDocument, DockerInstructionKind, DockerShellKind, DockerStageShell,
};

/// Parses JSON exec-array form `["cmd", "arg1", ...]` if `raw` is a valid JSON string array.
#[must_use]
pub fn parse_docker_command_form(raw: &str) -> DockerCommandForm {
    let trimmed = raw.trim();
    if trimmed.starts_with('[')
        && let Ok(argv) = serde_json::from_str::<Vec<String>>(trimmed)
    {
        return DockerCommandForm::ExecForm(argv);
    }
    DockerCommandForm::ShellForm
}

/// Classifies a `SHELL [...]` JSON argv vector into a [`DockerStageShell`].
#[must_use]
pub fn classify_shell_argv(argv: &[String]) -> DockerStageShell {
    let Some(exe) = argv.first() else {
        return DockerStageShell::default_posix();
    };
    let base = exe
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(exe)
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    let kind = match base.as_str() {
        "sh" | "dash" | "ash" | "busybox" => DockerShellKind::PosixSh,
        "bash" | "ksh" | "zsh" => DockerShellKind::Bash,
        "pwsh" | "powershell" => DockerShellKind::PowerShell,
        _ => DockerShellKind::Other,
    };
    let has_pipefail_flag = argv.windows(2).any(|w| {
        w[0].starts_with('-')
            && !w[0].starts_with("--")
            && w[0].ends_with('o')
            && w[1] == "pipefail"
    });
    let pipefail_enabled =
        matches!(kind, DockerShellKind::PowerShell | DockerShellKind::Other) || has_pipefail_flag;
    DockerStageShell {
        kind,
        argv: argv.to_vec(),
        pipefail_enabled,
    }
}

/// Parsed image name, optional tag, and optional digest from a `FROM` spec.
pub(super) struct ImageRefParts {
    /// Image repository/name without tag or digest.
    pub(super) image: String,
    /// Optional `:tag` suffix.
    pub(super) tag: Option<String>,
    /// Optional `@digest` suffix.
    pub(super) digest: Option<String>,
}

pub(super) fn split_image_ref(spec: &str) -> ImageRefParts {
    let (without_digest, digest) = spec.split_once('@').map_or_else(
        || (spec, None),
        |(img, dig)| (img, (!dig.is_empty()).then(|| String::from(dig))),
    );
    let last_slash = without_digest.rfind('/').unwrap_or(0);
    let tail = &without_digest[last_slash..];
    if let Some((head_rel, tag_str)) = tail.split_once(':') {
        let image = String::from(&without_digest[..last_slash + head_rel.len()]);
        let tag = (!tag_str.is_empty()).then(|| String::from(tag_str));
        ImageRefParts { image, tag, digest }
    } else {
        ImageRefParts {
            image: String::from(without_digest),
            tag: None,
            digest,
        }
    }
}

pub fn parse_non_empty(label: &str, args: &str) -> Result<String, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        Err(format!("{label} instruction requires arguments"))
    } else {
        Ok(String::from(trimmed))
    }
}

pub fn parse_env(args: &str) -> Result<DockerInstructionKind, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Err(String::from(
            "ENV instruction requires at least one key-value pair",
        ));
    }
    let mut bindings = Vec::new();
    if trimmed.contains('=') {
        for token in trimmed.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                bindings.push((String::from(k), String::from(v.trim_matches(['"', '\'']))));
            }
        }
    } else if let Some((k, v)) = trimmed.split_once(char::is_whitespace) {
        bindings.push((
            String::from(k.trim()),
            String::from(v.trim().trim_matches(['"', '\''])),
        ));
    } else {
        return Err(String::from("ENV instruction missing value"));
    }
    Ok(DockerInstructionKind::Env { bindings })
}

pub fn parse_arg(args: &str) -> Result<DockerInstructionKind, String> {
    let first = args
        .split_whitespace()
        .next()
        .ok_or_else(|| String::from("ARG instruction requires a variable name"))?;
    let (name, default_value) = first.split_once('=').map_or_else(
        || (String::from(first), None),
        |(k, v)| {
            (
                String::from(k),
                Some(String::from(v.trim_matches(['"', '\'']))),
            )
        },
    );
    if name.is_empty() {
        return Err(String::from("ARG variable name cannot be empty"));
    }
    Ok(DockerInstructionKind::Arg {
        name,
        default_value,
    })
}

pub fn parse_copy(rest: &str, doc: &DockerDocument) -> Result<DockerInstructionKind, String> {
    let arguments = parse_non_empty("COPY", rest)?;
    let mut copy_from = None;
    for tok in arguments.split_whitespace() {
        if !tok.starts_with("--") {
            break;
        }
        if let Some(head) = tok.get(..7)
            && head.eq_ignore_ascii_case("--from=")
        {
            copy_from = Some(String::from(&tok[7..]));
        }
    }
    let stage_index = doc.stages.len().saturating_sub(1);
    Ok(DockerInstructionKind::Copy {
        arguments,
        copy_from,
        stage_index,
    })
}

pub fn parse_add(rest: &str) -> Result<DockerInstructionKind, String> {
    let operands: Vec<String> = rest
        .split_whitespace()
        .filter(|t| !t.starts_with("--"))
        .map(String::from)
        .collect();
    if operands.len() < 2 {
        return Err(String::from("ADD requires source and destination operands"));
    }
    Ok(DockerInstructionKind::Add { operands })
}
