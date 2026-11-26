//! Tier 2 configuration auditing for `analysis_options.yaml` and `pubspec.yaml` (`GF081`).

use gf_core::{
    DartConfigIssue, RepoPath, Span, ToolConfigFinding, ToolConfigViolation, UastDocument,
};

fn push_dart_cfg(doc: &mut UastDocument, issue: DartConfigIssue, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.tool_config_findings.push(ToolConfigFinding {
        violation: ToolConfigViolation::Dart(issue),
        span: sp,
        message: String::from(msg),
    });
}

fn audit_analysis_options(source: &str, doc: &mut UastDocument) {
    let has_strict_casts = source.contains("strict-casts: true");
    let has_strict_raw = source.contains("strict-raw-types: true");
    let disables_ctx = source.contains("use_build_context_synchronously: false");
    if !has_strict_casts || !has_strict_raw || disables_ctx {
        push_dart_cfg(
            doc,
            DartConfigIssue::AnalysisOptionsWeakened,
            1,
            "`analysis_options.yaml` must enable `strict-casts: true`, `strict-raw-types: true`, and keep `use_build_context_synchronously` active",
        );
    }
}

fn audit_pubspec_yaml(source: &str, doc: &mut UastDocument) {
    let has_sdk = source.contains("environment:") && source.contains("sdk:");
    let has_any = source
        .lines()
        .any(|l| l.trim().ends_with(": any") || l.trim().ends_with(": \"any\""));
    if !has_sdk || has_any {
        push_dart_cfg(
            doc,
            DartConfigIssue::PubspecUnpinnedOrMissingSdk,
            1,
            "`pubspec.yaml` must pin `environment.sdk` and forbid unpinned `any` dependencies",
        );
    }
}

/// Audits `analysis_options.yaml` and `pubspec.yaml` for Tier 2 weakenings (`GF081`).
pub fn audit_dart_tool_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    match path.file_name() {
        "analysis_options.yaml" => audit_analysis_options(source, doc),
        "pubspec.yaml" => audit_pubspec_yaml(source, doc),
        _ => {}
    }
}
