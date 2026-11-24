//! Pure `#![no_std]` typed representation of `Dockerfile` stages, `SHELL` inheritance,
//! `ExecForm` vs `ShellForm` commands, and `COPY --from=...` stage targets.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Active command interpreter dialect inside a `Dockerfile` build stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockerShellKind {
    /// Default POSIX `/bin/sh` (`sh`, `dash`, `ash`, `busybox`).
    #[default]
    PosixSh,
    /// GNU Bourne-Again Shell (`bash`, `ksh`, `zsh`).
    Bash,
    /// PowerShell (`pwsh`, `powershell`, `powershell.exe`).
    PowerShell,
    /// Non-shell or alternative interpreter (`cmd`, `python`, `fish`, etc.).
    Other,
}

/// Syntactic invocation form of `RUN`, `CMD`, `ENTRYPOINT`, or `SHELL` instructions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockerCommandForm {
    /// JSON exec array `["executable", "arg1", ...]` (bypasses shell word-splitting).
    ExecForm(Vec<String>),
    /// Raw shell string or heredoc body evaluated by the stage's active `SHELL`.
    ShellForm,
}

/// Active `SHELL` configuration carried by a `Dockerfile` stage (inheritable via `FROM <alias>`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockerStageShell {
    /// Detected shell family (`PosixSh`, `Bash`, `PowerShell`, `Other`).
    pub kind: DockerShellKind,
    /// Parsed `SHELL` argv vector (defaults to `["/bin/sh", "-c"]`).
    pub argv: Vec<String>,
    /// Whether pipes are failure-safe by stage configuration (`-o pipefail` or `PowerShell`).
    pub pipefail_enabled: bool,
}

impl DockerStageShell {
    /// Default Docker stage shell (`["/bin/sh", "-c"]`, `PosixSh`, `pipefail_enabled = false`).
    #[must_use]
    pub fn default_posix() -> Self {
        Self {
            kind: DockerShellKind::PosixSh,
            argv: alloc::vec![String::from("/bin/sh"), String::from("-c")],
            pipefail_enabled: false,
        }
    }
}

impl Default for DockerStageShell {
    fn default() -> Self {
        Self::default_posix()
    }
}

/// Single build stage initiated by a `FROM` instruction inside a `Dockerfile`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FromStage {
    /// Base image repository name (e.g. `"rust"`, `"alpine"`, `"scratch"`, or stage alias).
    pub image: String,
    /// Optional image tag following `:` (e.g. `"1.88-slim"` or `"latest"`).
    pub tag: Option<String>,
    /// Optional immutable content digest following `@` (e.g. `"sha256:..."`).
    pub digest: Option<String>,
    /// Optional stage alias declared via `AS <alias>`.
    pub alias: Option<String>,
    /// Last `USER` value configured within this stage, if any.
    pub effective_user: Option<String>,
    /// Final active `SHELL` state of this stage (inherited by child `FROM <alias>` stages).
    #[serde(default)]
    pub effective_shell: DockerStageShell,
    /// Source span of the `FROM` instruction.
    pub span: Span,
}

/// Typed `Dockerfile` instruction categories inspected by `GF030`–`GF033` and `DK-DL*`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockerInstructionKind {
    /// `FROM` stage header.
    From {
        /// Zero-based index into [`DockerDocument::stages`].
        stage_index: usize,
    },
    /// `USER <user>[:<group>]` privilege switch.
    User {
        /// Raw user specification.
        user: String,
    },
    /// `SHELL ["executable", "param1", ...]` stage shell override.
    Shell {
        /// Parsed command form (`ExecForm` required by Docker spec).
        form: DockerCommandForm,
        /// Resulting stage shell state.
        shell: DockerStageShell,
    },
    /// `RUN` shell, heredoc, or JSON exec command.
    Run {
        /// Collapsed command body (excluding `RUN` keyword and `--mount`/`--network` flags).
        command: String,
        /// Syntactic invocation form (`ExecForm` vs `ShellForm`).
        form: DockerCommandForm,
        /// Whether this `RUN` contains an unguarded shell pipeline `|` (`DK-DL4006`).
        has_unguarded_pipe: bool,
    },
    /// `CMD` container default command (`DK-DL3025` checks `ExecForm` vs `ShellForm`).
    Cmd {
        /// Raw arguments text.
        arguments: String,
        /// Syntactic invocation form (`ExecForm` vs `ShellForm`).
        form: DockerCommandForm,
    },
    /// `ENTRYPOINT` container entrypoint (`DK-DL3025` checks `ExecForm` vs `ShellForm`).
    Entrypoint {
        /// Raw arguments text.
        arguments: String,
        /// Syntactic invocation form (`ExecForm` vs `ShellForm`).
        form: DockerCommandForm,
    },
    /// `COPY` file transfer instruction (`DK-DL3022` checks `--from=<stage>`).
    Copy {
        /// Raw arguments string.
        arguments: String,
        /// Optional `--from=<ref>` stage alias, numeric index, or external image.
        copy_from: Option<String>,
        /// Zero-based index of the current `FROM` stage containing this `COPY`.
        stage_index: usize,
    },
    /// `ADD` archive/remote/file instruction.
    Add {
        /// Space-split source and destination tokens (excluding `--*` flags).
        operands: Vec<String>,
    },
    /// `WORKDIR <path>` directory switch.
    Workdir {
        /// Target working directory path.
        path: String,
    },
    /// `ENV` key-value declarations.
    Env {
        /// Parsed `(key, value)` pairs.
        bindings: Vec<(String, String)>,
    },
    /// `ARG <name>[=<default>]` build-time parameter.
    Arg {
        /// Argument identifier.
        name: String,
        /// Optional hardcoded default value.
        default_value: Option<String>,
    },
    /// Any other recognized valid `Dockerfile` instruction (`EXPOSE`, `LABEL`, `VOLUME`, etc.).
    Other {
        /// Uppercase instruction keyword.
        keyword: String,
        /// Raw instruction arguments.
        arguments: String,
    },
}

/// Single parsed `Dockerfile` instruction paired with its exact inclusive line [`Span`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockerInstruction {
    /// Semantic kind and payload of the instruction.
    pub kind: DockerInstructionKind,
    /// Inclusive 1-indexed line span (accounting for `\` continuations and heredocs).
    pub span: Span,
}

/// Complete structured representation of a parsed `Dockerfile` / `Containerfile`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockerDocument {
    /// Ordered build stages (`FROM ...`).
    pub stages: Vec<FromStage>,
    /// Ordered sequence of parsed instructions.
    pub instructions: Vec<DockerInstruction>,
}

impl DockerDocument {
    /// Returns the active [`DockerShellKind`] of the current (latest) stage, defaulting to `PosixSh`.
    #[must_use]
    pub fn active_shell_kind(&self) -> DockerShellKind {
        self.stages
            .last()
            .map_or(DockerShellKind::PosixSh, |s| s.effective_shell.kind)
    }
}
