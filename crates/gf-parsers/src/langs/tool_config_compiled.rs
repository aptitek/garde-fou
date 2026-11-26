//! Tier 2 static configuration & manifest auditors for Go (`go.mod`, `.golangci.yml`),
//! Kotlin (`detekt.yml`, `build.gradle.kts`), and C# (`*.csproj`, `Directory.Build.props`, `.editorconfig`).

use gf_core::{
    CompiledConfigIssue, CompiledModuleSummary, RepoPath, Span, ToolConfigFinding,
    ToolConfigViolation, UastDocument,
};

use super::jsonc::{line_span_containing, push_cfg_violation};

const CRITICAL_CS_NOWARN: &[&str] = &["CS8618", "CS8600", "CS4014", "CA2007"];

fn line_span(line_no: u32) -> Span {
    let n = line_no.max(1);
    Span::new(n, n).unwrap_or(Span::FIRST)
}

fn push_compiled(
    out: &mut Vec<ToolConfigFinding>,
    issue: CompiledConfigIssue,
    span: Span,
    msg: impl Into<String>,
) {
    push_cfg_violation(out, ToolConfigViolation::Compiled(issue), span, msg.into());
}

fn audit_go_mod_line(line: &str, line_no: u32, out: &mut Vec<ToolConfigFinding>) {
    if line.contains("=>") && (line.contains("../") || line.contains("./")) {
        push_compiled(
            out,
            CompiledConfigIssue::GoModLocalReplace,
            line_span(line_no),
            String::from("local relative `replace` directive found in `go.mod`"),
        );
    }
}

fn audit_go_mod(source: &str, doc: &mut UastDocument) {
    let mut mod_path = None;
    let mut has_go_ver = false;
    for (idx, raw) in source.lines().enumerate() {
        let line = raw.split("//").next().unwrap_or(raw).trim();
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        if let Some(m) = line
            .strip_prefix("module ")
            .map(|r| r.trim().trim_matches('"'))
            .filter(|m| !m.is_empty())
        {
            mod_path = Some(String::from(m));
        }
        has_go_ver |= line.starts_with("go 1.") || line.starts_with("go 2.");
        audit_go_mod_line(line, line_no, &mut doc.tool_config_findings);
    }
    if !has_go_ver {
        push_compiled(
            &mut doc.tool_config_findings,
            CompiledConfigIssue::GoModMissingVersion,
            line_span(1),
            String::from("`go.mod` is missing an explicit `go 1.xx` version directive"),
        );
    }
    doc.compiled_meta = Some(CompiledModuleSummary {
        go_module_path: mod_path,
        ..CompiledModuleSummary::default()
    });
}

fn audit_golangci_yml(source: &str, out: &mut Vec<ToolConfigFinding>) {
    let lower = source.to_ascii_lowercase();
    let disables_all = lower.contains("disable-all: true") || lower.contains("default: none");
    let has_core =
        lower.contains("errcheck") || lower.contains("govet") || lower.contains("staticcheck");
    if disables_all && !has_core {
        let needle = if lower.contains("disable-all") {
            "disable-all"
        } else {
            "default: none"
        };
        push_compiled(
            out,
            CompiledConfigIssue::GolangciDisableAll,
            line_span_containing(source, needle),
            String::from(
                "`.golangci.yml` disables default linters without enabling `errcheck`/`govet`/`staticcheck`",
            ),
        );
    }
    if lower.contains("tests: false") {
        push_compiled(
            out,
            CompiledConfigIssue::GolangciTestsDisabled,
            line_span_containing(source, "tests: false"),
            String::from("`.golangci.yml` sets `tests: false`, skipping Go test files"),
        );
    }
}

fn audit_detekt_yml(source: &str, out: &mut Vec<ToolConfigFinding>) {
    let mut section = "";
    for (idx, raw) in source.lines().enumerate() {
        let trimmed = raw.trim();
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        if !raw.starts_with(' ') && !raw.starts_with('\t') && trimmed.ends_with(':') {
            section = trimmed.trim_end_matches(':');
        }
        if let Some(rest) = trimmed.strip_prefix("maxIssues:")
            && rest.trim().parse::<u32>().is_ok_and(|n| n > 0)
        {
            push_compiled(
                out,
                CompiledConfigIssue::DetektMaxIssuesNonZero,
                line_span(line_no),
                format!("`detekt.yml` sets `maxIssues: {}` (must be 0)", rest.trim()),
            );
        }
        if matches!(section, "coroutines" | "exceptions") && trimmed == "active: false" {
            push_compiled(
                out,
                CompiledConfigIssue::DetektCategoryDisabled,
                line_span(line_no),
                format!("`detekt.yml` disables `{section}` rule set"),
            );
        }
    }
}

fn audit_gradle_build(source: &str, out: &mut Vec<ToolConfigFinding>) {
    for (idx, raw) in source.lines().enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let span = line_span(line_no);
        if raw.contains(":+\"") || raw.contains(":+'") || raw.contains("latest.release") {
            push_compiled(
                out,
                CompiledConfigIssue::GradleDynamicVersion,
                span,
                String::from(
                    "dynamic Gradle dependency version (`+` / `latest.release`) breaks reproducibility",
                ),
            );
        }
        if raw.contains("allowInsecureProtocol = true")
            || (raw.contains("maven") && raw.contains("http://"))
        {
            push_compiled(
                out,
                CompiledConfigIssue::GradleInsecureRepo,
                span,
                String::from(
                    "insecure plaintext `http://` Maven repository or `allowInsecureProtocol = true`",
                ),
            );
        }
    }
}

fn audit_csproj_xml(source: &str, out: &mut Vec<ToolConfigFinding>) {
    let Ok(xml) = roxmltree::Document::parse(source) else {
        return;
    };
    for node in xml.descendants().filter(roxmltree::Node::is_element) {
        let tag = node.tag_name().name();
        let val = node.text().unwrap_or("").trim();
        let pos = xml.text_pos_at(node.range().start);
        let span = line_span(pos.row);
        match tag {
            "Nullable" if val.eq_ignore_ascii_case("disable") => push_compiled(
                out,
                CompiledConfigIssue::CsprojNullableDisabled,
                span,
                String::from("`<Nullable>disable</Nullable>` weakens C# null-safety"),
            ),
            "TreatWarningsAsErrors" if val.eq_ignore_ascii_case("false") => push_compiled(
                out,
                CompiledConfigIssue::CsprojTreatWarningsFalse,
                span,
                String::from(
                    "`<TreatWarningsAsErrors>false</TreatWarningsAsErrors>` weakens compiler gates",
                ),
            ),
            "EnableNETAnalyzers" if val.eq_ignore_ascii_case("false") => push_compiled(
                out,
                CompiledConfigIssue::CsprojAnalyzersDisabled,
                span,
                String::from(
                    "`<EnableNETAnalyzers>false</EnableNETAnalyzers>` disables built-in Roslyn analyzers",
                ),
            ),
            "NoWarn" => {
                for code in CRITICAL_CS_NOWARN {
                    if val.contains(code) {
                        push_compiled(
                            out,
                            CompiledConfigIssue::CsprojCriticalNoWarn,
                            span,
                            format!("`<NoWarn>` suppresses critical diagnostic `{code}`"),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn audit_editorconfig(source: &str, out: &mut Vec<ToolConfigFinding>) {
    for (idx, raw) in source.lines().enumerate() {
        let lower = raw.trim().to_ascii_lowercase();
        if lower.starts_with("dotnet_analyzer_diagnostic.severity")
            && (lower.ends_with("none") || lower.ends_with("silent"))
        {
            let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            push_compiled(
                out,
                CompiledConfigIssue::EditorConfigAnalyzerSilenced,
                line_span(line_no),
                String::from(
                    "`.editorconfig` globally silences `dotnet_analyzer_diagnostic.severity`",
                ),
            );
        }
    }
}

/// Dispatches Tier 2 manifest & tool-config auditing (`GF081`) for Go, Kotlin, and C# files.
pub fn audit_compiled_tool_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let base = path.file_name();
    match base {
        "go.mod" => audit_go_mod(source, doc),
        ".golangci.yml" | ".golangci.yaml" => {
            audit_golangci_yml(source, &mut doc.tool_config_findings);
        }
        "detekt.yml" | "detekt.yaml" => audit_detekt_yml(source, &mut doc.tool_config_findings),
        "build.gradle.kts" | "build.gradle" => {
            audit_gradle_build(source, &mut doc.tool_config_findings);
        }
        "Directory.Build.props" => audit_csproj_xml(source, &mut doc.tool_config_findings),
        ".editorconfig" => audit_editorconfig(source, &mut doc.tool_config_findings),
        _ if base.ends_with(".csproj") => audit_csproj_xml(source, &mut doc.tool_config_findings),
        _ => {}
    }
}
