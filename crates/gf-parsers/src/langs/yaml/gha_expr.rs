//! GitHub Actions `${{ ... }}` expression normalization (`GHA-TEMPLATE-INJECTION`),
//! `pull_request_target` trigger detection, and `uses:` ref pinning (`GHA-PERMISSIONS-AND-PINNING`).

use gf_core::{LangRuleFinding, Span, WorkflowViolation};

use super::anchors_merge::ResolvedYaml;
use super::events_tree::YamlScalar;

const UNTRUSTED_GHA_PREFIXES: &[&str] = &[
    "github.head_ref",
    "github.event.issue.title",
    "github.event.issue.body",
    "github.event.pull_request.title",
    "github.event.pull_request.body",
    "github.event.pull_request.head.ref",
    "github.event.pull_request.head.label",
    "github.event.pull_request.head.repo.default_branch",
    "github.event.comment.body",
    "github.event.review.body",
    "github.event.review_comment.body",
    "github.event.discussion.title",
    "github.event.discussion.body",
    "github.event.pages.",
    "github.event.commits.",
    "github.event.head_commit.message",
    "github.event.head_commit.author.",
];

const FORBIDDEN_MUTABLE_REFS: &[&str] = &["main", "master", "head", "latest"];

fn push_normalized_bracket(out: &mut String, bracket_inner: &str) {
    let trimmed = bracket_inner.trim().trim_matches('\'').trim_matches('"');
    out.push('.');
    if trimmed.bytes().all(|b| b.is_ascii_digit()) {
        out.push('*');
    } else {
        out.push_str(&trimmed.to_ascii_lowercase());
    }
}

/// Normalizes bracket-notation (`github['event'][0]['title']`) into dotted lowercase form.
#[must_use]
pub fn normalize_gha_expr(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '[' {
            let bracket_inner: String = chars.by_ref().take_while(|&c| c != ']').collect();
            push_normalized_bracket(&mut out, &bracket_inner);
        } else {
            out.push(ch.to_ascii_lowercase());
        }
    }
    out
}

fn find_untrusted_context(expr: &str) -> Option<&'static str> {
    let norm = normalize_gha_expr(expr);
    UNTRUSTED_GHA_PREFIXES
        .iter()
        .copied()
        .find(|&prefix| norm.contains(prefix))
}

/// Scans `run:` or `actions/github-script` `with.script` scalars for untrusted `${{ ... }}` sinks.
pub fn scan_scalar_template_injections(
    scalar: &YamlScalar,
    sink_label: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut segs = scalar.value.split("${{");
    let Some(first) = segs.next() else {
        return;
    };
    let mut seen_newlines = first.bytes().filter(|&b| b == b'\n').count();
    for seg in segs {
        let Some((expr, after_close)) = seg.split_once("}}") else {
            break;
        };
        if let Some(ctx_name) = find_untrusted_context(expr) {
            let line_offset = u32::try_from(seen_newlines).unwrap_or(0).saturating_add(1);
            let line = scalar.remap_embedded_line(line_offset);
            let span = Span::new(line, line).unwrap_or(scalar.span);
            out.push(LangRuleFinding::new(
                WorkflowViolation::TemplateInjection,
                span,
                format!(
                    "untrusted `${{{{ ... {ctx_name} ... }}}}` interpolated directly into `{sink_label}`"
                ),
            ));
        }
        seen_newlines = seen_newlines
            .saturating_add(expr.bytes().filter(|&b| b == b'\n').count())
            .saturating_add(after_close.bytes().filter(|&b| b == b'\n').count());
    }
}

/// Returns `true` when root `on:` enables `pull_request_target`.
#[must_use]
pub fn has_pull_request_target_trigger(root: &ResolvedYaml) -> bool {
    let Some(on_node) = root.get("on") else {
        return false;
    };
    match on_node {
        ResolvedYaml::Scalar(s) => s.value.trim() == "pull_request_target",
        ResolvedYaml::Sequence { items, .. } => items
            .iter()
            .filter_map(ResolvedYaml::as_scalar)
            .any(|s| s.value.trim() == "pull_request_target"),
        ResolvedYaml::Mapping { entries, .. } => entries
            .iter()
            .any(|(k, _)| k.value == "pull_request_target"),
    }
}

/// Enforces explicit version tag or commit SHA on non-local `uses: owner/repo@ref`.
pub fn check_uses_pinning(uses_scalar: &YamlScalar, out: &mut Vec<LangRuleFinding>) {
    let raw = uses_scalar.value.trim();
    if raw.is_empty()
        || raw.starts_with("./")
        || raw.starts_with(".\\")
        || raw.starts_with("docker://")
    {
        return;
    }
    let Some((action_slug, git_ref)) = raw.rsplit_once('@') else {
        out.push(LangRuleFinding::new(
            WorkflowViolation::PermissionsOrUnpinnedUses,
            uses_scalar.span,
            format!("action `{raw}` is missing an explicit `@<tag-or-sha>` version ref"),
        ));
        return;
    };
    let ref_trim = git_ref.trim();
    if action_slug.is_empty()
        || ref_trim.is_empty()
        || FORBIDDEN_MUTABLE_REFS
            .iter()
            .any(|bad| ref_trim.eq_ignore_ascii_case(bad))
    {
        out.push(LangRuleFinding::new(
            WorkflowViolation::PermissionsOrUnpinnedUses,
            uses_scalar.span,
            format!("action `{raw}` tracks mutable branch ref `@{ref_trim}`"),
        ));
    }
}
