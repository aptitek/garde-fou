//! Pure `#![no_std]` typed representation of `Dockerfile` stages and instructions.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

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
    /// Source span of the `FROM` instruction.
    pub span: Span,
}

/// Typed `Dockerfile` instruction categories inspected by `GF030`–`GF033`.
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
    /// `RUN <command>` shell or exec command.
    Run {
        /// Command body (excluding `RUN` keyword).
        command: String,
    },
    /// `COPY` file transfer instruction.
    Copy {
        /// Raw arguments string.
        arguments: String,
    },
    /// `ADD` archive/remote/file instruction.
    Add {
        /// Space-split source and destination tokens (excluding `--chown` flags).
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
    /// Any other recognized valid `Dockerfile` instruction (`EXPOSE`, `CMD`, `ENTRYPOINT`, etc.).
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
