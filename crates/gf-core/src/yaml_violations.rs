//! Pure `#![no_std]` closed sum types for YAML 1.2 (`YAML-*`), Docker Compose (`COMPOSE-*`),
//! and GitHub Actions (`GHA-*`) Tier 1 violations (`GF080`), plus [`YamlFlavor`] path routing.

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::path::RepoPath;

/// Specialized sub-ecosystem of a `.yml` / `.yaml` document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum YamlFlavor {
    /// Docker Compose specification (`compose.yaml`, `docker-compose*.yml`).
    DockerCompose {
        /// `true` for `*.override.yml` / `*.override.yaml` partial service overlays.
        is_override: bool,
    },
    /// GitHub Actions workflow (`.github/workflows/*.y*ml`) or composite action (`.github/actions/**/action.y*ml`).
    GithubWorkflow,
    /// Generic YAML 1.2 configuration (`Taskfile.yml`, `lefthook.yml`, Kubernetes, `OpenAPI`, etc.).
    GenericYaml,
}

fn has_yaml_ext(lower_name: &str) -> bool {
    lower_name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| matches!(ext, "yml" | "yaml"))
}

fn is_compose_basename(lower_name: &str) -> bool {
    if !has_yaml_ext(lower_name) {
        return false;
    }
    matches!(
        lower_name,
        "compose.yml" | "compose.yaml" | "docker-compose.yml" | "docker-compose.yaml"
    ) || lower_name.starts_with("compose.")
        || lower_name.starts_with("docker-compose.")
}

fn is_gha_workflow_or_action_path(lower_path: &str, lower_name: &str) -> bool {
    if !has_yaml_ext(lower_name) {
        return false;
    }
    let in_workflows = lower_path
        .strip_prefix(".github/workflows/")
        .is_some_and(|rest| !rest.contains('/'));
    let in_actions = lower_path.starts_with(".github/actions/")
        && matches!(lower_name, "action.yml" | "action.yaml");
    in_workflows || in_actions
}

/// Classifies a repository-relative `.yml`/`.yaml` path into its [`YamlFlavor`].
#[must_use]
pub fn classify_yaml_flavor(path: &RepoPath) -> YamlFlavor {
    let lower_path = path.as_str().to_ascii_lowercase();
    let lower_name = path.file_name().to_ascii_lowercase();
    if is_gha_workflow_or_action_path(&lower_path, &lower_name) {
        return YamlFlavor::GithubWorkflow;
    }
    if is_compose_basename(&lower_name) {
        return YamlFlavor::DockerCompose {
            is_override: lower_name.contains(".override."),
        };
    }
    YamlFlavor::GenericYaml
}

/// Closed sum type of native YAML 1.2 & legacy YAML 1.1 compatibility violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum YamlViolation {
    /// Duplicate key in the same mapping or conflicting `<<: [*a, *b]` merge (`YAML-DUPLICATE-KEY`).
    DuplicateKey,
    /// Ambiguous YAML 1.1 plain boolean (`yes`/`no`/`on`/`off`/`y`/`n`/`True`/`False`, `YAML-TRUTHY`).
    TruthyAmbiguity,
    /// Implicit YAML 1.1 octal integer `0755` instead of `"0755"` or `0o755` (`YAML-IMPLICIT-OCTAL`).
    ImplicitOctal,
    /// Unquoted sexagesimal `22:22` / `1:20:30` silently parsed as base-60 in YAML 1.1 (`YAML-SEXAGESIMAL`).
    SexagesimalValue,
    /// Excessive alias expansion depth or node count (`YAML-ANCHOR-BOMB`).
    AnchorBomb,
}

const YAML_CODES: [&str; 5] = [
    "YAML-DUPLICATE-KEY",
    "YAML-TRUTHY",
    "YAML-IMPLICIT-OCTAL",
    "YAML-SEXAGESIMAL",
    "YAML-ANCHOR-BOMB",
];

const YAML_REMEDIATIONS: [&str; 5] = [
    "Remove duplicate mapping keys (or explicitly override conflicting `<<` merge keys in the child mapping) so earlier entries are never silently overwritten.",
    "Use canonical lowercase `true` or `false` booleans, or quote string literals (`\"yes\"`, `\"on\"`, `\"y\"`) to prevent YAML 1.1 vs 1.2 coercion bugs.",
    "Quote file-mode strings (`\"0755\"`) or use explicit YAML 1.2 octal syntax (`0o755`) instead of ambiguous leading-zero `0755`.",
    "Quote colon-separated scalars (`\"22:22\"`, `\"1:20:30\"`) so YAML 1.1 parsers never evaluate them as base-60 integers.",
    "Flatten deeply nested YAML `&anchor` / `*alias` chains (`<= 16` levels, `<= 2048` expanded nodes) to prevent billion-laughs DoS.",
];

impl YamlViolation {
    /// Canonical ecosystem rule identifier (`"YAML-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        YAML_CODES[self as usize]
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        YAML_REMEDIATIONS[self as usize]
    }
}

impl fmt::Display for YamlViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Closed sum type of native Docker Compose security & topology violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ComposeViolation {
    /// `privileged: true`, host namespace (`network_mode`/`pid`/`ipc: host`), `user: root|0`, dangerous `cap_add`, or `unconfined` (`COMPOSE-PRIVILEGED-OR-HOST`).
    PrivilegedOrHostNamespace,
    /// Mounting `/var/run/docker.sock` or `/run/docker.sock` into a container (`COMPOSE-DOCKER-SOCKET`).
    DockerSocketMount,
    /// Missing `image`+`build`, `:latest`, untagged image, or `${TAG:-latest}` (`COMPOSE-UNPINNED-IMAGE`).
    UnpinnedOrMissingImage,
    /// Unquoted short-syntax port mapping `- 8080:80` / `- 22:22` (`COMPOSE-UNQUOTED-PORTS`).
    UnquotedPortMapping,
    /// Hardcoded plaintext credential inside `environment:` (`COMPOSE-SECRET-IN-ENV`).
    PlaintextSecretInEnv,
    /// Missing target service, dependency cycle, or `service_healthy` without `healthcheck` (`COMPOSE-BROKEN-DEPENDS-ON`).
    BrokenDependsOnGraph,
    /// Undeclared named volume/network/secret/config or unused top-level resource (`COMPOSE-ORPHAN-OR-UNDECLARED-RESOURCE`).
    OrphanOrUndeclaredResource,
}

const COMPOSE_CODES: [&str; 7] = [
    "COMPOSE-PRIVILEGED-OR-HOST",
    "COMPOSE-DOCKER-SOCKET",
    "COMPOSE-UNPINNED-IMAGE",
    "COMPOSE-UNQUOTED-PORTS",
    "COMPOSE-SECRET-IN-ENV",
    "COMPOSE-BROKEN-DEPENDS-ON",
    "COMPOSE-ORPHAN-OR-UNDECLARED-RESOURCE",
];

const COMPOSE_REMEDIATIONS: [&str; 7] = [
    "Run containers unprivileged in isolated namespaces under a non-root `user:` without `ALL`/`SYS_ADMIN` capabilities or `unconfined` profiles.",
    "Never mount `docker.sock` directly into application containers; use a read-only socket proxy or rootless daemon API.",
    "Declare an explicit version-tagged or `@sha256:`-pinned `image:` (without `:latest` fallbacks) or a `build:` section on every base service.",
    "Quote short-syntax port mappings (`\"8080:80\"`, `\"127.0.0.1:5432:5432\"`) or use long port syntax.",
    "Pass sensitive credentials via Compose `secrets:`, `env_file:`, host passthrough (`- SECRET_KEY`), or `${SECRET_KEY:?}` interpolation.",
    "Ensure `depends_on` references existing acyclic services and that `condition: service_healthy` targets define an active `healthcheck:`.",
    "Declare every named volume, network, secret, and config at the Compose root and remove unused top-level resource declarations.",
];

impl ComposeViolation {
    /// Canonical ecosystem rule identifier (`"COMPOSE-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        COMPOSE_CODES[self as usize]
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        COMPOSE_REMEDIATIONS[self as usize]
    }
}

impl fmt::Display for ComposeViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Closed sum type of native GitHub Actions workflow security violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WorkflowViolation {
    /// Untrusted `${{ github.event... }}` interpolated directly into `run:` or `github-script` (`GHA-TEMPLATE-INJECTION`).
    TemplateInjection,
    /// `pull_request_target` combined with `actions/checkout` of PR head (`GHA-PWN-REQUEST`).
    PwnRequestCheckout,
    /// Missing/excessive `permissions:` or mutable branch `uses: owner/repo@main` (`GHA-PERMISSIONS-AND-PINNING`).
    PermissionsOrUnpinnedUses,
}

const GHA_CODES: [&str; 3] = [
    "GHA-TEMPLATE-INJECTION",
    "GHA-PWN-REQUEST",
    "GHA-PERMISSIONS-AND-PINNING",
];

const GHA_REMEDIATIONS: [&str; 3] = [
    "Bind untrusted `${{ github.event... }}` expressions to step `env:` variables and reference `\"$ENV_VAR\"` / `process.env.ENV_VAR` instead of inline script interpolation.",
    "Never check out `github.event.pull_request.head.sha` or `github.head_ref` inside a privileged `pull_request_target` workflow.",
    "Declare minimal `permissions: { contents: read }` at workflow root (elevating write scopes per job only) and pin `uses:` to explicit release tags or 40-char commit SHAs.",
];

impl WorkflowViolation {
    /// Canonical ecosystem rule identifier (`"GHA-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        GHA_CODES[self as usize]
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        GHA_REMEDIATIONS[self as usize]
    }
}

impl fmt::Display for WorkflowViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
