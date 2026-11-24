//! Deterministic `skills.sh` remote skill catalog and `[agents]` configuration (`GF070`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::context::{FeatureId, FrameworkId, LanguageId, ProjectContext};
use crate::policies::ScannerPolicy;

/// `[agents]` configuration governing `AGENTS.md` synchronization and `skills.sh` installation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentsConfig {
    /// Enforcement mode for `AGENTS.md` and `.agents/skills/garde-fou/SKILL.md` (`GF070`).
    pub sync_policy: ScannerPolicy,
    /// Whether `garde init` and `garde sync` may install contextual `skills.sh` skills.
    pub remote_skills: bool,
    /// Additional `skills.sh` entries formatted as `"owner/repo@skill"`.
    pub extra_skills: Vec<String>,
    /// Skill IDs explicitly excluded from automatic installation.
    pub disabled_skills: Vec<String>,
}

impl AgentsConfig {
    /// Default agentic configuration (`sync_policy = Auto`, `remote_skills = true`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            sync_policy: ScannerPolicy::Auto,
            remote_skills: true,
            extra_skills: Vec::new(),
            disabled_skills: Vec::new(),
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "ag(sp={},rs={},ex={:?},dis={:?})",
            self.sync_policy.as_str(),
            self.remote_skills,
            self.extra_skills,
            self.disabled_skills
        )
    }
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Synchronization status of `AGENTS.md` and `.agents/skills/garde-fou/SKILL.md` (`GF070`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgenticSyncStatus {
    /// `sync_policy = "off"` or `sync_policy = "auto"` without an opted-in managed block.
    #[default]
    NotApplicable,
    /// Both `AGENTS.md` managed block and `.agents/skills/garde-fou/SKILL.md` match exactly.
    Synced,
    /// `sync_policy = "required"`, and `AGENTS.md` (or its managed block) is missing.
    MissingAgentsMd,
    /// `AGENTS.md` managed block drifted from workspace context, config thresholds, or skills.
    DriftedAgentsMd,
    /// `.agents/skills/garde-fou/SKILL.md` is missing or outdated.
    MissingOrOutdatedCoreSkill,
}

/// Compile-time metadata for a verified skill in the `skills.sh` ecosystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteSkillSpec {
    /// Canonical directory slug under `.agents/skills/<id>/`.
    pub id: &'static str,
    /// GitHub `<owner>/<repo>` hosting the skill.
    pub repo: &'static str,
    /// Preferred relative directory path inside `repo` containing `SKILL.md`.
    pub skill_path: &'static str,
    /// Public `skills.sh` catalog URL.
    pub skills_sh_url: &'static str,
    /// One-line description surfaced in `AGENTS.md`.
    pub summary: &'static str,
}

/// Owned resolved skill specification (from built-in catalog or `extra_skills`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSkillSpec {
    /// Skill folder name under `.agents/skills/<id>/`.
    pub id: String,
    /// GitHub `<owner>/<repo>` slug.
    pub repo: String,
    /// Candidate relative directory inside `repo`.
    pub skill_path: String,
    /// Public `skills.sh` reference URL.
    pub skills_sh_url: String,
    /// Concise capability summary.
    pub summary: String,
}

impl From<RemoteSkillSpec> for ResolvedSkillSpec {
    fn from(s: RemoteSkillSpec) -> Self {
        Self {
            id: String::from(s.id),
            repo: String::from(s.repo),
            skill_path: String::from(s.skill_path),
            skills_sh_url: String::from(s.skills_sh_url),
            summary: String::from(s.summary),
        }
    }
}

/// Installed skill entry indexed in the generated `AGENTS.md` table.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SkillEntry {
    /// Skill directory name under `.agents/skills/<id>/SKILL.md`.
    pub id: String,
    /// Short human/agent summary of when to load this skill.
    pub summary: String,
}

const SKILL_PBT: RemoteSkillSpec = RemoteSkillSpec {
    id: "property-based-testing",
    repo: "trailofbits/skills",
    skill_path: "plugins/property-based-testing/skills/property-based-testing",
    skills_sh_url: "https://skills.sh/trailofbits/skills/property-based-testing",
    summary: "Property-based testing invariants (proptest, Hypothesis, fast-check).",
};

const SKILL_AST_GREP: RemoteSkillSpec = RemoteSkillSpec {
    id: "ast-grep",
    repo: "ast-grep/agent-skill",
    skill_path: "skills/ast-grep",
    skills_sh_url: "https://skills.sh/ast-grep/agent-skill/ast-grep",
    summary: "Structural AST search and refactoring rules with ast-grep.",
};

const SKILL_RUST_BP: RemoteSkillSpec = RemoteSkillSpec {
    id: "rust-best-practices",
    repo: "apollographql/skills",
    skill_path: "skills/rust-best-practices",
    skills_sh_url: "https://skills.sh/apollographql/skills/rust-best-practices",
    summary: "Idiomatic Safe Rust ownership, Clippy, and error-handling patterns.",
};

const SKILL_RUST_REVIEW: RemoteSkillSpec = RemoteSkillSpec {
    id: "rust-review",
    repo: "trailofbits/skills",
    skill_path: "plugins/rust-review/skills/rust-review",
    skills_sh_url: "https://skills.sh/trailofbits/skills/rust-review",
    summary: "Security & panic-DoS auditing for Rust crates and workspaces.",
};

const SKILL_WEB_DESIGN: RemoteSkillSpec = RemoteSkillSpec {
    id: "web-design-guidelines",
    repo: "vercel-labs/agent-skills",
    skill_path: "skills/web-design-guidelines",
    skills_sh_url: "https://skills.sh/vercel-labs/agent-skills/web-design-guidelines",
    summary: "UI accessibility (a11y), design tokens, and semantic HTML guidelines.",
};

const SKILL_REACT_BP: RemoteSkillSpec = RemoteSkillSpec {
    id: "vercel-react-best-practices",
    repo: "vercel-labs/agent-skills",
    skill_path: "skills/react-best-practices",
    skills_sh_url: "https://skills.sh/vercel-labs/agent-skills/vercel-react-best-practices",
    summary: "React & Next.js component composition and rendering performance.",
};

/// Parses a custom `"owner/repo@skill"` string from `[agents] extra_skills`.
///
/// # Errors
///
/// Returns an error string if the format is not `<owner>/<repo>@<skill>`.
pub fn parse_extra_skill_spec(raw: &str) -> Result<ResolvedSkillSpec, &'static str> {
    let trimmed = raw.trim();
    let Some((repo, skill_id)) = trimmed.split_once('@') else {
        return Err("expected `owner/repo@skill` format in `agents.extra_skills`");
    };
    let valid_repo = repo.split_once('/').is_some_and(|(o, r)| {
        !o.is_empty() && !r.is_empty() && !r.contains('/') && !repo.contains("..")
    });
    let valid_id = !skill_id.is_empty()
        && skill_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid_repo || !valid_id {
        return Err("invalid repository or skill slug in `agents.extra_skills`");
    }
    Ok(ResolvedSkillSpec {
        id: String::from(skill_id),
        repo: String::from(repo),
        skill_path: format!("skills/{skill_id}"),
        skills_sh_url: format!("https://skills.sh/{repo}/{skill_id}"),
        summary: format!("Custom skill `{skill_id}` from `{repo}`."),
    })
}

/// Resolves the ordered, deduplicated list of `skills.sh` skills matching `ctx` and `cfg`.
#[must_use]
pub fn select_contextual_skills(
    ctx: &ProjectContext,
    cfg: &AgentsConfig,
) -> Vec<ResolvedSkillSpec> {
    let disabled: BTreeSet<&str> = cfg.disabled_skills.iter().map(String::as_str).collect();
    let mut map: BTreeMap<String, ResolvedSkillSpec> = BTreeMap::new();
    let mut insert = |spec: ResolvedSkillSpec| {
        if !disabled.contains(spec.id.as_str()) {
            map.entry(spec.id.clone()).or_insert(spec);
        }
    };

    insert(SKILL_PBT.into());
    insert(SKILL_AST_GREP.into());
    if ctx.languages.contains(&LanguageId::Rust) {
        insert(SKILL_RUST_BP.into());
        insert(SKILL_RUST_REVIEW.into());
    }
    if ctx.languages.contains(&LanguageId::TypeScript) || ctx.features.contains(&FeatureId::Ui) {
        insert(SKILL_WEB_DESIGN.into());
    }
    if ctx.frameworks.contains(&FrameworkId::React) || ctx.frameworks.contains(&FrameworkId::NextJs)
    {
        insert(SKILL_REACT_BP.into());
    }
    for extra in &cfg.extra_skills {
        if let Ok(parsed) = parse_extra_skill_spec(extra) {
            insert(parsed);
        }
    }
    map.into_values().collect()
}
