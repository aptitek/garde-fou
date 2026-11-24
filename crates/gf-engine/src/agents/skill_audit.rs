//! Supply-chain security and guardrail-integrity auditor for `.agents/skills/` (`GF040` / `GF070`).

use std::fs;
use std::path::Path;

use gf_core::RepoPath;
use gf_parsers::parse_source;

const SABOTAGE_PATTERNS: &[(&str, &str)] = &[
    (
        "--no-verify",
        "attempts to bypass Git hooks (`--no-verify`)",
    ),
    ("rm -rf", "contains destructive `rm -rf` command"),
    ("rm garde-fou.toml", "attempts to delete `garde-fou.toml`"),
    (
        "sync_policy = \"off\"",
        "attempts to disable `agents.sync_policy`",
    ),
    (
        "require_lock = false",
        "attempts to disable `integrity.require_lock`",
    ),
];

/// Verified metadata extracted from a safe skill directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditedSkill {
    /// Concise one-line description extracted from `SKILL.md`.
    pub summary: String,
}

fn has_piped_remote_shell(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let downloads = lower.contains("curl ") || lower.contains("wget ");
    let pipes_shell = lower.contains("| sh")
        || lower.contains("|sh")
        || lower.contains("| bash")
        || lower.contains("|bash");
    downloads && pipes_shell
}

fn audit_text_file(rel_name: &str, content: &str) -> Result<(), String> {
    let probe_path = RepoPath::new(rel_name).unwrap_or_else(|_| RepoPath::agents_md());
    if let Ok(doc) = parse_source(&probe_path, content)
        && let Some(sec) = doc.secret_findings.first()
    {
        return Err(format!(
            "secret `{}` ({}) detected in `{rel_name}`",
            sec.kind, sec.redacted_preview
        ));
    }
    for line in content.lines() {
        for &(needle, reason) in SABOTAGE_PATTERNS {
            if line.contains(needle) {
                return Err(format!("{reason} in `{rel_name}`"));
            }
        }
        if has_piped_remote_shell(line) {
            return Err(format!(
                "contains unverified piped remote shell execution in `{rel_name}`"
            ));
        }
    }
    Ok(())
}

fn walk_and_audit_dir(base: &Path, dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("unreadable skill directory: {e}"))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("metadata error: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "symlinks are forbidden inside skills (`{}`)",
                path.display()
            ));
        }
        if meta.is_dir() {
            walk_and_audit_dir(base, &path)?;
        } else if meta.is_file()
            && let Ok(text) = fs::read_to_string(&path)
        {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            audit_text_file(&rel, &text)?;
        }
    }
    Ok(())
}

fn parse_description_value<'a>(rest: &'a str, next: Option<&'a str>) -> Option<&'a str> {
    let inline = rest.trim().trim_matches(['"', '\'']);
    if !inline.is_empty() && !matches!(inline, ">" | ">-" | "|") {
        return Some(inline);
    }
    let next_clean = next?.trim();
    (!next_clean.is_empty() && next_clean != "---").then(|| next_clean.trim_matches(['"', '\'']))
}

fn extract_frontmatter_description(lines: &[&str]) -> Option<String> {
    if lines.first().copied() != Some("---") {
        return None;
    }
    let end = lines
        .iter()
        .skip(1)
        .position(|&l| l == "---")
        .map_or(lines.len(), |p| p + 1);
    for idx in 1..end {
        if let Some(rest) = lines[idx].strip_prefix("description:")
            && let Some(val) = parse_description_value(rest, lines.get(idx + 1).copied())
        {
            return Some(truncate_summary(val));
        }
    }
    None
}

/// Extracts a concise human/agent summary from `SKILL.md` frontmatter or prose.
#[must_use]
pub fn extract_skill_summary(raw_skill_md: &str, fallback_id: &str) -> String {
    let lines: Vec<&str> = raw_skill_md
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    if let Some(desc) = extract_frontmatter_description(&lines) {
        return desc;
    }
    lines
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with('#') && *l != "---")
        .map_or_else(|| format!("Agent skill `{fallback_id}`."), truncate_summary)
}

fn truncate_summary(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let head: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

/// Audits `skill_dir` before allowing installation into `.agents/skills/<id>/`.
///
/// # Errors
///
/// Returns a human-readable rejection reason if `SKILL.md` is missing/empty or unsafe.
pub fn audit_skill_directory(skill_dir: &Path, skill_id: &str) -> Result<AuditedSkill, String> {
    let skill_md_path = skill_dir.join("SKILL.md");
    let meta = fs::symlink_metadata(&skill_md_path)
        .map_err(|_| format!("skill `{skill_id}` is missing `SKILL.md`"))?;
    if !meta.is_file() {
        return Err(format!("`SKILL.md` in `{skill_id}` is not a regular file"));
    }
    let content = fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("failed to read `{skill_id}/SKILL.md`: {e}"))?;
    if content.chars().filter(|c| !c.is_whitespace()).count() < 20 {
        return Err(format!("`{skill_id}/SKILL.md` is empty or trivially short"));
    }
    walk_and_audit_dir(skill_dir, skill_dir)?;
    Ok(AuditedSkill {
        summary: extract_skill_summary(&content, skill_id),
    })
}
