//! Deterministic `skills.sh` remote skill catalog and `[agents]` configuration (`GF070`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::archetype_policy::WorkspaceArchetype;
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
    /// Optional pinned Git commit SHA (`owner/repo@skill#commit_sha`).
    pub pinned_rev: Option<String>,
}

impl From<RemoteSkillSpec> for ResolvedSkillSpec {
    fn from(s: RemoteSkillSpec) -> Self {
        Self {
            id: String::from(s.id),
            repo: String::from(s.repo),
            skill_path: String::from(s.skill_path),
            skills_sh_url: format!("https://skills.sh/{}/{}", s.repo, s.id),
            summary: String::from(s.summary),
            pinned_rev: None,
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

#[rustfmt::skip]
const BUILTINS: [(&str, &str, &str, &str); 11] = [
    ("property-based-testing", "trailofbits/skills", "plugins/property-based-testing/skills/property-based-testing", "Property-based testing invariants (proptest, Hypothesis, fast-check)."),
    ("ast-grep", "ast-grep/agent-skill", "skills/ast-grep", "Structural AST search and refactoring rules with ast-grep."),
    ("rust-best-practices", "apollographql/skills", "skills/rust-best-practices", "Idiomatic Safe Rust ownership, Clippy, and error-handling patterns."),
    ("rust-review", "trailofbits/skills", "plugins/rust-review/skills/rust-review", "Security & panic-DoS auditing for Rust crates and workspaces."),
    ("web-design-guidelines", "vercel-labs/agent-skills", "skills/web-design-guidelines", "UI accessibility (a11y), design tokens, and semantic HTML guidelines."),
    ("vercel-react-best-practices", "vercel-labs/agent-skills", "skills/react-best-practices", "React & Next.js component composition and rendering performance."),
    ("workers-best-practices", "cloudflare/skills", "skills/workers-best-practices", "Cloudflare Workers, D1, R2, and Wrangler production edge best practices."),
    ("modern-python", "trailofbits/skills", "plugins/modern-python/skills/modern-python", "Modern Python workflows with uv, ruff, pytest, and strict type checking."),
    ("obsidian-markdown", "kepano/obsidian-skills", "plugins/obsidian/skills/obsidian-markdown", "Obsidian Flavored Markdown wikilinks, embeds, callouts, and properties."),
    ("obsidian-bases", "kepano/obsidian-skills", "plugins/obsidian/skills/obsidian-bases", "Obsidian Bases structured view and filter definitions."),
    ("json-canvas", "kepano/obsidian-skills", "plugins/obsidian/skills/json-canvas", "JSON Canvas 1.0 (.canvas) node and edge graph authoring."),
];

/// Returns `true` when `skill_id` is part of the built-in Garde-fou `skills.sh` catalog
/// (excluding user-authored custom skills under `.agents/skills/`).
#[must_use]
pub fn is_catalog_skill_id(skill_id: &str) -> bool {
    skill_id == "garde-fou" || BUILTINS.iter().any(|(id, _, _, _)| *id == skill_id)
}

#[rustfmt::skip]
fn builtin_spec(idx: usize) -> ResolvedSkillSpec {
    let (id, repo, skill_path, summary) = BUILTINS[idx];
    RemoteSkillSpec { id, repo, skill_path, summary }.into()
}

fn split_pinned_rev(tail: &str) -> Result<(&str, Option<String>), &'static str> {
    let Some((id, sha)) = tail.split_once('#') else {
        return Ok((tail, None));
    };
    if (7..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok((id, Some(String::from(sha))))
    } else {
        Err("invalid commit SHA in `agents.extra_skills` (expected 7..=64 hex)")
    }
}

fn is_valid_repo_and_skill(repo: &str, skill_id: &str) -> bool {
    let valid_repo = repo.split_once('/').is_some_and(|(o, r)| {
        !o.is_empty() && !r.is_empty() && !r.contains('/') && !repo.contains("..")
    });
    let valid_id = !skill_id.is_empty()
        && skill_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    valid_repo && valid_id
}

/// Parses `"owner/repo@skill"` or `"owner/repo@skill#commit_sha"` from `[agents] extra_skills`.
///
/// # Errors
///
/// Returns an error string if the slug or optional hex commit SHA is malformed.
pub fn parse_extra_skill_spec(raw: &str) -> Result<ResolvedSkillSpec, &'static str> {
    let Some((repo, tail)) = raw.trim().split_once('@') else {
        return Err("expected `owner/repo@skill[#sha]` format in `agents.extra_skills`");
    };
    let (skill_id, pinned_rev) = split_pinned_rev(tail)?;
    if !is_valid_repo_and_skill(repo, skill_id) {
        return Err("invalid repository or skill slug in `agents.extra_skills`");
    }
    Ok(ResolvedSkillSpec {
        id: String::from(skill_id),
        repo: String::from(repo),
        skill_path: format!("skills/{skill_id}"),
        skills_sh_url: format!("https://skills.sh/{repo}/{skill_id}"),
        summary: format!("Custom skill `{skill_id}` from `{repo}`."),
        pinned_rev,
    })
}

#[rustfmt::skip]
fn has_web_ui_context(ctx: &ProjectContext) -> bool {
    [LanguageId::TypeScript, LanguageId::Astro, LanguageId::Html].iter().any(|l| ctx.languages.contains(l))
        || [FeatureId::Ui, FeatureId::PandaCss, FeatureId::Material3Expressive, FeatureId::W3c]
            .iter()
            .any(|f| ctx.features.contains(f))
}

#[rustfmt::skip]
fn add_framework_skills(ctx: &ProjectContext, insert: &mut impl FnMut(ResolvedSkillSpec)) {
    if has_web_ui_context(ctx) { insert(builtin_spec(4)); }
    if [FrameworkId::React, FrameworkId::NextJs, FrameworkId::ReactRouter].iter().any(|fw| ctx.frameworks.contains(fw)) {
        insert(builtin_spec(5));
    }
    if ctx.frameworks.contains(&FrameworkId::CloudflareWorkers) { insert(builtin_spec(6)); }
    if ctx.frameworks.contains(&FrameworkId::Obsidian) || ctx.archetype == WorkspaceArchetype::Knowledge {
        for idx in [8, 9, 10] { insert(builtin_spec(idx)); }
    }
}

#[rustfmt::skip]
fn add_archetype_skills(ctx: &ProjectContext, insert: &mut impl FnMut(ResolvedSkillSpec)) {
    if ctx.archetype == WorkspaceArchetype::Minimal { return; }
    if ctx.archetype == WorkspaceArchetype::Software { insert(builtin_spec(0)); insert(builtin_spec(1)); }
    if ctx.languages.contains(&LanguageId::Rust) { insert(builtin_spec(2)); insert(builtin_spec(3)); }
    if ctx.languages.contains(&LanguageId::Python) || ctx.frameworks.contains(&FrameworkId::Jupyter) { insert(builtin_spec(7)); }
    add_framework_skills(ctx, insert);
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
    add_archetype_skills(ctx, &mut insert);
    for extra in &cfg.extra_skills {
        if let Ok(parsed) = parse_extra_skill_spec(extra) {
            insert(parsed);
        }
    }
    map.into_values().collect()
}
