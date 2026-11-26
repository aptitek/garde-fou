//! Legacy CLI command scanner (`npm`/`yarn`/`pip`/`poetry`/`pipenv`) and Gradle Kotlin DSL
//! hardcoded dependency extractor (`GF081`).

use gf_core::{Span, ToolConfigFinding, ToolConfigViolation, ToolchainViolation};

use super::jsonc::push_cfg_violation;

fn line_span(line_no: u32) -> Span {
    let n = line_no.max(1);
    Span::new(n, n).unwrap_or(Span::FIRST)
}

fn push_tc(out: &mut Vec<ToolConfigFinding>, v: ToolchainViolation, span: Span, msg: String) {
    push_cfg_violation(out, ToolConfigViolation::Toolchain(v), span, msg);
}

fn strip_cmd_prefix(seg: &str) -> &str {
    let mut s = seg.trim_start();
    for p in ["RUN ", "- ", "run:", "cmd:", "command:"] {
        if let Some(rest) = s.strip_prefix(p) {
            s = rest.trim_start();
        }
    }
    s.trim_start_matches(['"', '\''])
}

fn legacy_pnpm_cmd_on_line(line: &str) -> bool {
    for seg in line.split(['&', ';', '|']) {
        let s = strip_cmd_prefix(seg);
        if [
            "npm install",
            "npm ci",
            "npm run",
            "npm exec",
            "npx ",
            "yarn install",
            "yarn add",
            "yarn run",
        ]
        .iter()
        .any(|p| s.starts_with(p))
        {
            return true;
        }
    }
    false
}

fn legacy_uv_cmd_on_line(line: &str) -> bool {
    if line.contains("!pip install") || line.contains("%pip install") {
        return true;
    }
    for seg in line.split(['&', ';', '|']) {
        let s = strip_cmd_prefix(seg);
        if s.starts_with("uv ") {
            continue;
        }
        if [
            "pip install",
            "pip3 install",
            "python -m pip",
            "python3 -m pip",
            "poetry install",
            "poetry run",
            "pipenv ",
            "!pip install",
            "%pip install",
        ]
        .iter()
        .any(|p| s.starts_with(p))
        {
            return true;
        }
    }
    false
}

/// Extracts hardcoded `"group:artifact:version"` string literals from `build.gradle.kts`
/// `dependencies { ... }` blocks for workspace-level `GRADLE-USE-VERSION-CATALOG`.
#[must_use]
pub fn extract_gradle_hardcoded_deps(source: &str) -> Vec<(Span, String)> {
    let mut out = Vec::new();
    let mut in_deps = false;
    let mut depth: i32 = 0;
    for (idx, raw) in source.lines().enumerate() {
        let trimmed = raw.split("//").next().unwrap_or(raw).trim();
        if trimmed.starts_with("dependencies") && trimmed.contains('{') {
            in_deps = true;
            depth = 1;
            continue;
        }
        if !in_deps {
            continue;
        }
        let opens = i32::try_from(trimmed.bytes().filter(|&b| b == b'{').count()).unwrap_or(0);
        let closes = i32::try_from(trimmed.bytes().filter(|&b| b == b'}').count()).unwrap_or(0);
        depth += opens - closes;
        if depth <= 0 {
            in_deps = false;
            continue;
        }
        if let Some((_, after_q)) = trimmed.split_once('"')
            && let Some((coord, _)) = after_q.split_once('"')
            && coord.matches(':').count() >= 2
        {
            let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            out.push((line_span(line_no), String::from(coord)));
        }
    }
    out
}

/// Runs [`audit_legacy_cli_commands`] when `path`/`lang` is a command-bearing file
/// (`Dockerfile`, `Taskfile.yml`, `lefthook.yml`, GitHub Actions workflows, shell/PS1, notebooks).
pub fn maybe_audit_legacy_cli_commands(
    lang: gf_core::Language,
    path: &gf_core::RepoPath,
    source: &str,
    doc: &mut gf_core::UastDocument,
) {
    let is_yaml_cmd = lang == gf_core::Language::Yaml
        && (matches!(
            path.file_name(),
            "Taskfile.yml" | "Taskfile.yaml" | "lefthook.yml" | "lefthook.yaml"
        ) || path.as_str().starts_with(".github/workflows/"));
    if is_yaml_cmd
        || matches!(
            lang,
            gf_core::Language::Dockerfile
                | gf_core::Language::Shell
                | gf_core::Language::PowerShell
                | gf_core::Language::Notebook
                | gf_core::Language::Quarto
        )
    {
        audit_legacy_cli_commands(source, &mut doc.tool_config_findings);
    }
}

/// Scans command-bearing text (`Dockerfile`, `Taskfile.yml`, CI workflows, shell/PS1, notebooks)
/// for legacy `npm`/`yarn` and `pip`/`poetry`/`pipenv` invocations (`GF081`).
fn audit_legacy_cli_commands(source: &str, out: &mut Vec<ToolConfigFinding>) {
    for (idx, raw) in source.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        if legacy_pnpm_cmd_on_line(trimmed) {
            push_tc(
                out,
                ToolchainViolation::PnpmOnly,
                line_span(line_no),
                String::from("legacy `npm`/`yarn` command detected; use `pnpm` exclusively"),
            );
        }
        if legacy_uv_cmd_on_line(trimmed) {
            push_tc(
                out,
                ToolchainViolation::UvOnly,
                line_span(line_no),
                String::from(
                    "legacy `pip`/`poetry`/`pipenv` command detected; use `uv` exclusively",
                ),
            );
        }
    }
}
