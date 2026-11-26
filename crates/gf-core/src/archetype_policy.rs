//! Proportional Arsenal workspace archetypes (`WorkspaceArchetype`) and pure
//! rule-gating policy matrix (`ArchetypePolicy`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::domains::{DomainKind, DomainLayoutConfig, classify_path};
use crate::glob::glob_matches;
use crate::path::RepoPath;

/// High-level workspace archetype driving proportional governance without weakening
/// intrinsic AST, syntax, complexity, or secret checks on present files.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WorkspaceArchetype {
    /// Bare governance workspace with zero assumed programming language.
    Minimal,
    /// Practical automation/installer script repository (`install.sh`, `setup.ps1`, PEP 723 `.py`).
    Scripts,
    /// Infrastructure & container stack repository (`compose.yaml`, `Dockerfile`, `stacks/`).
    Infra,
    /// Obsidian vault or pure Markdown/JSON Canvas knowledge base.
    Knowledge,
    /// Academic / data-science repository (Jupyter `.ipynb` notebooks & Quarto `.qmd` courses).
    Academic,
    /// Full software engineering workspace enforcing strict 6-domain segregation.
    #[default]
    Software,
}

#[rustfmt::skip]
const ARCHETYPE_NAMES: [&str; 6] = ["minimal", "scripts", "infra", "knowledge", "academic", "software"];

#[rustfmt::skip]
const ARCHETYPE_ALIASES: &[(&str, WorkspaceArchetype)] = &[
    ("minimal", WorkspaceArchetype::Minimal), ("bare", WorkspaceArchetype::Minimal),
    ("scripts", WorkspaceArchetype::Scripts), ("script", WorkspaceArchetype::Scripts),
    ("infra", WorkspaceArchetype::Infra), ("ops", WorkspaceArchetype::Infra),
    ("knowledge", WorkspaceArchetype::Knowledge), ("obsidian", WorkspaceArchetype::Knowledge),
    ("vault", WorkspaceArchetype::Knowledge), ("academic", WorkspaceArchetype::Academic),
    ("notebook", WorkspaceArchetype::Academic), ("notebooks", WorkspaceArchetype::Academic),
    ("quarto", WorkspaceArchetype::Academic), ("software", WorkspaceArchetype::Software),
    ("app", WorkspaceArchetype::Software),
];

impl WorkspaceArchetype {
    /// All [`WorkspaceArchetype`] variants in deterministic order.
    pub const ALL: [Self; 6] = [
        Self::Minimal,
        Self::Scripts,
        Self::Infra,
        Self::Knowledge,
        Self::Academic,
        Self::Software,
    ];

    /// Parses a case-insensitive archetype slug from CLI or `garde-fou.toml`.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        ARCHETYPE_ALIASES
            .iter()
            .find(|(alias, _)| raw.eq_ignore_ascii_case(alias))
            .map(|(_, id)| *id)
    }

    /// Canonical lowercase archetype identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        ARCHETYPE_NAMES[self as usize]
    }
}

impl fmt::Display for WorkspaceArchetype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[project] archetype` configuration setting (`"auto"` or an explicit [`WorkspaceArchetype`]).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchetypeSetting {
    /// Automatically infer the workspace archetype from repository files and frameworks.
    #[default]
    Auto,
    /// Force a specific [`WorkspaceArchetype`].
    Explicit(WorkspaceArchetype),
}

impl ArchetypeSetting {
    /// Parses `"auto"` or any [`WorkspaceArchetype`] slug.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("auto") {
            return Some(Self::Auto);
        }
        WorkspaceArchetype::from_config_str(raw).map(Self::Explicit)
    }

    /// Canonical lowercase label for cache keys and serialization.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Explicit(arch) => arch.as_str(),
        }
    }

    /// Resolves standalone archetype (defaults to [`WorkspaceArchetype::Software`] when `Auto`).
    #[must_use]
    pub const fn resolve_standalone(self) -> WorkspaceArchetype {
        match self {
            Self::Auto => WorkspaceArchetype::Software,
            Self::Explicit(arch) => arch,
        }
    }
}

/// Enforcement mode for `GF065` script test coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptTestGate {
    /// Every script in `Scripts` requires automated test coverage (`Software`).
    AlwaysRequired,
    /// Require script tests only when a `tests/` suite exists or `FeatureId::Tdd` is active.
    OnlyWhenTestsOrTddPresent,
}

/// Enforcement mode for `GF021` dead-export checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadExportGate {
    /// Enforce dead-export analysis on `Code` modules.
    Enforced,
    /// Skip dead-export analysis (e.g. non-library knowledge/minimal workspaces).
    Disabled,
}

/// Pure proportional-arsenal policy derived from a [`WorkspaceArchetype`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchetypePolicy {
    /// Active workspace archetype.
    pub archetype: WorkspaceArchetype,
    /// Policy for `GF065` script test coverage.
    pub script_test_gate: ScriptTestGate,
    /// Policy for `GF021` dead exports.
    pub dead_export_gate: DeadExportGate,
}

impl ArchetypePolicy {
    /// Returns the deterministic proportional policy for `archetype`.
    #[must_use]
    pub const fn for_archetype(archetype: WorkspaceArchetype) -> Self {
        match archetype {
            WorkspaceArchetype::Software => Self {
                archetype,
                script_test_gate: ScriptTestGate::AlwaysRequired,
                dead_export_gate: DeadExportGate::Enforced,
            },
            WorkspaceArchetype::Scripts | WorkspaceArchetype::Infra => Self {
                archetype,
                script_test_gate: ScriptTestGate::OnlyWhenTestsOrTddPresent,
                dead_export_gate: DeadExportGate::Enforced,
            },
            WorkspaceArchetype::Minimal
            | WorkspaceArchetype::Knowledge
            | WorkspaceArchetype::Academic => Self {
                archetype,
                script_test_gate: ScriptTestGate::OnlyWhenTestsOrTddPresent,
                dead_export_gate: DeadExportGate::Disabled,
            },
        }
    }

    /// Returns `true` when `GF065` should require test coverage for `DomainKind::Scripts`.
    #[must_use]
    pub const fn requires_script_test(self, has_tests_or_tdd: bool) -> bool {
        match self.script_test_gate {
            ScriptTestGate::AlwaysRequired => true,
            ScriptTestGate::OnlyWhenTestsOrTddPresent => has_tests_or_tdd,
        }
    }

    /// Returns `true` when `print(...)` calls are legitimate CLI/notebook output (`PY-T201` exempt).
    #[must_use]
    pub const fn allows_print_in_domain(self, domain: DomainKind) -> bool {
        match self.archetype {
            WorkspaceArchetype::Scripts | WorkspaceArchetype::Infra => {
                matches!(domain, DomainKind::Scripts | DomainKind::Meta)
            }
            WorkspaceArchetype::Academic => true,
            WorkspaceArchetype::Minimal
            | WorkspaceArchetype::Knowledge
            | WorkspaceArchetype::Software => false,
        }
    }
}

#[rustfmt::skip]
const KNOWLEDGE_ASSET_GLOBS: &[&str] = &["assets/**", "attachments/**", "files/**", "images/**", "public/**", "static/**"];
#[rustfmt::skip]
const ACADEMIC_ASSET_GLOBS: &[&str] = &["assets/**", "images/**", "figures/**", "public/**", "static/**"];

fn has_ext_in(path: &RepoPath, exts: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| exts.iter().any(|cand| e.eq_ignore_ascii_case(cand)))
}

fn classify_content_arch(path: &RepoPath, assets: &[&str], exts: &[&str]) -> Option<DomainKind> {
    if assets.iter().any(|g| glob_matches(g, path.as_str())) {
        Some(DomainKind::Asset)
    } else if has_ext_in(path, exts) {
        Some(DomainKind::Contenus)
    } else {
        None
    }
}

fn classify_ops_arch(path: &RepoPath, archetype: WorkspaceArchetype) -> Option<DomainKind> {
    let raw = path.as_str();
    if raw.starts_with("bin/") {
        return Some(DomainKind::Scripts);
    }
    if archetype == WorkspaceArchetype::Scripts {
        let root_script = !raw.contains('/') && has_ext_in(path, &["sh", "bash", "ps1", "py"]);
        return root_script.then_some(DomainKind::Scripts);
    }
    ["services/", "stacks/", "deploy/"]
        .iter()
        .any(|p| raw.starts_with(p))
        .then_some(DomainKind::Meta)
}

/// Classifies `path` into its [`DomainKind`] taking `archetype` proportionality into account.
#[must_use]
#[rustfmt::skip]
pub fn classify_path_for_archetype(
    path: &RepoPath,
    layout: &DomainLayoutConfig,
    archetype: WorkspaceArchetype,
) -> DomainKind {
    let raw = path.as_str();
    if raw.starts_with(".agents/") || raw.starts_with(".obsidian/") { return DomainKind::Meta; }
    let special = match archetype {
        WorkspaceArchetype::Knowledge => classify_content_arch(path, KNOWLEDGE_ASSET_GLOBS, &["md", "canvas", "base"]),
        WorkspaceArchetype::Academic => classify_content_arch(path, ACADEMIC_ASSET_GLOBS, &["ipynb", "qmd", "md"]),
        WorkspaceArchetype::Scripts | WorkspaceArchetype::Infra => classify_ops_arch(path, archetype),
        WorkspaceArchetype::Minimal | WorkspaceArchetype::Software => None,
    };
    special.unwrap_or_else(|| classify_path(path, layout))
}
