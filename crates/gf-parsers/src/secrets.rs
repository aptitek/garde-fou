//! Zero-dependency native secret & private-key detector with mandatory redaction.

use gf_core::{RepoPath, SecretFinding, Span, glob_matches};

const TEST_PATH_GLOBS: &[&str] = &[
    "tests/**",
    "**/tests/**",
    "*.test.*",
    "**/*.test.*",
    "*.spec.*",
    "**/*.spec.*",
    "*_test.*",
    "**/*_test.*",
    "test_*.py",
    "**/test_*.py",
];

struct TokenSignature {
    prefix: &'static str,
    min_tail: usize,
    kind: &'static str,
    preview: &'static str,
}

impl TokenSignature {
    const fn new(
        prefix: &'static str,
        min_tail: usize,
        kind: &'static str,
        preview: &'static str,
    ) -> Self {
        Self {
            prefix,
            min_tail,
            kind,
            preview,
        }
    }
}

const TOKEN_SIGNATURES: &[TokenSignature] = &[
    TokenSignature::new("ghp_", 20, "github-pat", "ghp_****"),
    TokenSignature::new(
        "github_pat_",
        20,
        "github-fine-grained-pat",
        "github_pat_****",
    ),
    TokenSignature::new("glpat-", 16, "gitlab-pat", "glpat-****"),
    TokenSignature::new("sk_live_", 16, "stripe-live-key", "sk_live_****"),
    TokenSignature::new("xoxb-", 15, "slack-bot-token", "xoxb-****"),
    TokenSignature::new("xoxa-", 15, "slack-app-token", "xoxa-****"),
    TokenSignature::new("xoxp-", 15, "slack-user-token", "xoxp-****"),
];

const SENSITIVE_ASSIGN_KEYS: &[&str] = &[
    "password",
    "passwd",
    "api_key",
    "apikey",
    "private_key",
    "access_token",
    "auth_token",
    "client_secret",
];

const PLACEHOLDER_MARKERS: &[&str] = &[
    "example",
    "changeme",
    "dummy",
    "placeholder",
    "redacted",
    "replace_me",
    "your_",
    "todo",
    "****",
];

fn is_dedicated_test_file(path: &RepoPath) -> bool {
    let raw = path.as_str();
    TEST_PATH_GLOBS.iter().any(|pat| glob_matches(pat, raw))
}

fn has_pem_private_key_header(line: &str) -> bool {
    let start_marker = concat!("-----", "BEGIN ");
    let end_marker = concat!("PRIVATE ", "KEY-----");
    let Some((_, after_start)) = line.split_once(start_marker) else {
        return false;
    };
    let Some((label, _)) = after_start.split_once(end_marker) else {
        return false;
    };
    label.len() <= 16 && label.chars().all(|c| c.is_ascii_uppercase() || c == ' ')
}

fn match_aws_access_key(token: &str) -> Option<(&'static str, &'static str)> {
    let tail = token.strip_prefix("AKIA")?;
    let upper_len = tail
        .chars()
        .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        .count();
    (upper_len == 16).then_some(("aws-access-key-id", "AKIA****************"))
}

fn token_tail_len(tail: &str) -> usize {
    tail.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .count()
}

fn match_token_prefix(token: &str) -> Option<(&'static str, &'static str)> {
    for sig in TOKEN_SIGNATURES {
        if let Some(tail) = token.strip_prefix(sig.prefix)
            && token_tail_len(tail) >= sig.min_tail
        {
            return Some((sig.kind, sig.preview));
        }
    }
    match_aws_access_key(token)
}

fn detect_signature_token(line: &str) -> Option<(&'static str, &'static str)> {
    line.split(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '=' | ':' | ',' | ';' | '(' | ')')
    })
    .find_map(match_token_prefix)
}

fn is_sensitive_key_name(lhs: &str) -> bool {
    let ident = lhs
        .trim()
        .trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("");
    let lower = ident.to_ascii_lowercase();
    SENSITIVE_ASSIGN_KEYS
        .iter()
        .any(|&k| lower == k || lower.ends_with(&format!("_{k}")))
}

fn is_suspect_literal_value(rhs: &str) -> bool {
    let trimmed = rhs.trim().trim_end_matches([',', ';']);
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        });
    let Some(val) = unquoted else {
        return false;
    };
    if val.len() < 8 || val.starts_with('$') || val.starts_with('<') || val.contains("${") {
        return false;
    }
    let lower = val.to_ascii_lowercase();
    !PLACEHOLDER_MARKERS.iter().any(|m| lower.contains(m))
}

fn detect_suspicious_assignment(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
        return false;
    }
    let Some((lhs, rhs)) = trimmed.split_once('=').or_else(|| trimmed.split_once(':')) else {
        return false;
    };
    if rhs.starts_with('=') {
        return false;
    }
    is_sensitive_key_name(lhs) && is_suspect_literal_value(rhs)
}

/// Scans `source` line-by-line and returns redacted [`SecretFinding`] items (`GF040`).
pub fn extract_secret_findings(path: &RepoPath, source: &str) -> Vec<SecretFinding> {
    let check_assignments = !is_dedicated_test_file(path);
    let mut findings = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
        let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);

        if has_pem_private_key_header(line) {
            findings.push(SecretFinding {
                kind: String::from("pem-private-key"),
                redacted_preview: String::from("-----BEGIN [REDACTED] PRIVATE KEY-----"),
                span,
            });
        } else if let Some((kind, preview)) = detect_signature_token(line) {
            findings.push(SecretFinding {
                kind: String::from(kind),
                redacted_preview: String::from(preview),
                span,
            });
        } else if check_assignments && detect_suspicious_assignment(line) {
            findings.push(SecretFinding {
                kind: String::from("hardcoded-secret-assignment"),
                redacted_preview: String::from("<redacted-secret:****>"),
                span,
            });
        }
    }
    findings
}
