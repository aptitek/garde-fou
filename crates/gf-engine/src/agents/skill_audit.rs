//! Supply-chain security and guardrail-integrity auditor for `.agents/skills/` (`GF040` / `GF070`).

use std::fs;
use std::path::Path;

use gf_core::RepoPath;
use gf_parsers::parse_source;

pub use super::verifier::extract_skill_summary;

const COMPACT_TOML_SABOTAGE: &[(&str, &str)] = &[
    (
        "require_lock=false",
        "attempts to disable `integrity.require_lock`",
    ),
    (
        "sync_policy=\"off\"",
        "attempts to disable `agents.sync_policy`",
    ),
    (
        "sync_policy='off'",
        "attempts to disable `agents.sync_policy`",
    ),
    (
        "forbid_unwrap=false",
        "attempts to disable `rules.forbid_unwrap`",
    ),
];

const LINE_SABOTAGE_PATTERNS: &[(&str, &str)] = &[
    (
        "--no-verify",
        "attempts to bypass Git hooks (`--no-verify`)",
    ),
    ("rm -rf", "contains destructive `rm -rf` command"),
    ("rm -fr", "contains destructive `rm -fr` command"),
    ("rm -r -f", "contains destructive `rm -r -f` command"),
    ("rm -f -r", "contains destructive `rm -f -r` command"),
    ("rm garde-fou.toml", "attempts to delete `garde-fou.toml`"),
    ("rm garde.lock", "attempts to delete `garde.lock`"),
];

const HTML_COMMENT_INJECTION_MARKERS: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "disregard previous",
    "system prompt",
    "--no-verify",
    "require_lock",
    "sync_policy",
    "forbid_unwrap",
    "garde.lock",
    "garde-fou.toml",
    "curl ",
    "wget ",
    "rm -",
];

/// Verified metadata extracted from a safe skill directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditedSkill {
    /// Concise one-line description extracted from `SKILL.md`.
    pub summary: String,
}

fn check_stealth_unicode(rel_name: &str, content: &str) -> Result<(), String> {
    for (byte_idx, ch) in content.char_indices() {
        let code = u32::from(ch);
        if matches!(ch, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
            return Err(format!(
                "Bidi Trojan Source control character U+{code:04X} detected in `{rel_name}`"
            ));
        }
        if matches!(ch, '\u{200B}'..='\u{200F}') || (ch == '\u{FEFF}' && byte_idx > 0) {
            return Err(format!(
                "zero-width invisible character U+{code:04X} detected in `{rel_name}`"
            ));
        }
        if matches!(ch, '\u{E0000}'..='\u{E007F}') {
            return Err(format!(
                "hidden Unicode Tag character U+{code:05X} detected in `{rel_name}`"
            ));
        }
    }
    Ok(())
}

fn has_git_commit_short_no_verify(line: &str) -> bool {
    let tokens: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '`'))
        .filter(|t| !t.is_empty())
        .collect();
    tokens.windows(2).any(|w| w == ["git", "commit"]) && tokens.contains(&"-n")
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

fn check_html_comments(rel_name: &str, content: &str) -> Result<(), String> {
    let mut rest = content;
    while let Some((_, after_open)) = rest.split_once("<!--") {
        let (comment_body, next_rest) = after_open.split_once("-->").unwrap_or((after_open, ""));
        let lower = comment_body.to_ascii_lowercase();
        if let Some(&marker) = HTML_COMMENT_INJECTION_MARKERS
            .iter()
            .find(|&&m| lower.contains(m))
        {
            return Err(format!(
                "suspicious hidden HTML comment directive (`{marker}`) in `{rel_name}`"
            ));
        }
        rest = next_rest;
    }
    Ok(())
}

fn audit_single_line(rel_name: &str, line: &str) -> Result<(), String> {
    if let Some(&(_, reason)) = LINE_SABOTAGE_PATTERNS
        .iter()
        .find(|&&(needle, _)| line.contains(needle))
    {
        return Err(format!("{reason} in `{rel_name}`"));
    }
    if has_git_commit_short_no_verify(line) {
        return Err(format!(
            "attempts to bypass Git hooks (`git commit -n`) in `{rel_name}`"
        ));
    }
    if has_piped_remote_shell(line) {
        return Err(format!(
            "contains unverified piped remote shell execution in `{rel_name}`"
        ));
    }
    Ok(())
}

fn audit_text_file(rel_name: &str, content: &str) -> Result<(), String> {
    check_stealth_unicode(rel_name, content)?;
    check_html_comments(rel_name, content)?;

    let probe_path =
        RepoPath::new(rel_name).map_err(|e| format!("invalid skill path `{rel_name}`: {e}"))?;
    if let Ok(doc) = parse_source(&probe_path, content)
        && let Some(sec) = doc.secret_findings.first()
    {
        return Err(format!(
            "secret `{}` ({}) detected in `{rel_name}`",
            sec.kind, sec.redacted_preview
        ));
    }

    let compact: String = content
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if let Some(&(_, reason)) = COMPACT_TOML_SABOTAGE
        .iter()
        .find(|&&(needle, _)| compact.contains(needle))
    {
        return Err(format!("{reason} in `{rel_name}`"));
    }

    for line in content.lines() {
        audit_single_line(rel_name, line)?;
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
