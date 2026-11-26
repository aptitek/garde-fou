//! Tier 2 configuration auditing for `composer.json`, `phpstan.neon`, and `psalm.xml` (`GF081`).

use gf_core::{
    PhpConfigIssue, RepoPath, Span, ToolConfigFinding, ToolConfigViolation, UastDocument,
};

fn push_cfg(doc: &mut UastDocument, issue: PhpConfigIssue, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.tool_config_findings.push(ToolConfigFinding {
        violation: ToolConfigViolation::Php(issue),
        span: sp,
        message: String::from(msg),
    });
}

fn audit_composer_json(source: &str, doc: &mut UastDocument) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    let has_php = val
        .get("require")
        .and_then(|r| r.get("php"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let has_psr4 = val
        .get("autoload")
        .and_then(|a| a.get("psr-4"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|o| !o.is_empty());
    if !has_php || !has_psr4 {
        push_cfg(
            doc,
            PhpConfigIssue::ComposerMissingPhpOrPsr4,
            1,
            "`composer.json` must pin `require.php` and declare `autoload.psr-4`",
        );
    }
}

fn parse_leading_u32(s: &str) -> Option<u32> {
    let digits: String = s
        .trim()
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse::<u32>().ok()
}

fn audit_phpstan_neon(source: &str, doc: &mut UastDocument) {
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        let trimmed = line.trim();
        let low_level = trimmed
            .strip_prefix("level:")
            .and_then(parse_leading_u32)
            .is_some_and(|n| n < 8);
        if low_level || trimmed.contains("#.*#") || trimmed.contains("'/.*/'") {
            push_cfg(
                doc,
                PhpConfigIssue::PhpstanLevelTooLowOrBlanketIgnore,
                ln,
                "`phpstan.neon` must use `level: 8`/`max` without wildcard `ignoreErrors`",
            );
        }
    }
}

fn audit_psalm_xml(source: &str, doc: &mut UastDocument) {
    let weak = source.lines().enumerate().find_map(|(idx, line)| {
        let (_, tail) = line.split_once("errorLevel=")?;
        let lv = parse_leading_u32(tail)?;
        (lv > 2).then_some(u32::try_from(idx + 1).unwrap_or(1))
    });
    if let Some(ln) = weak {
        push_cfg(
            doc,
            PhpConfigIssue::PsalmErrorLevelTooWeak,
            ln,
            "`psalm.xml` `errorLevel` must be <= 2",
        );
    }
}

/// Audits `composer.json`, `phpstan.neon`, and `psalm.xml` for Tier 2 weakenings (`GF081`).
pub fn audit_php_tool_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let name = path.file_name();
    if name == "composer.json" {
        audit_composer_json(source, doc);
    } else if name.starts_with("phpstan.neon") {
        audit_phpstan_neon(source, doc);
    } else if name.starts_with("psalm.xml") {
        audit_psalm_xml(source, doc);
    }
}
