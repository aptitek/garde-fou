//! Tier 3 external runners for Go (`golangci-lint` v2/v1, `go vet`), Kotlin (`detekt` SARIF,
//! `ktlint`), and C# (`dotnet build`) with 0 ms `--staged` short-circuiting (`GF082`).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use gf_core::{Diagnostic, LanguageId, RepoPath, ScannerPolicy};
use gf_parsers::{
    parse_go_vet_json, parse_golangci_lint_json, parse_ktlint_json, parse_msbuild_diagnostics,
};
use gf_rules::convert_external_tool_findings;

use super::AdapterCtx;
use super::python_runners::resolve_system_binary;
use crate::scanners::gitleaks::missing_tool_diagnostic;
use crate::scanners::sarif_ingest::{SarifMapping, ingest_sarif_json};
use crate::types::EngineError;

fn effective(pol: ScannerPolicy, strict: bool) -> ScannerPolicy {
    if strict && pol == ScannerPolicy::Auto {
        ScannerPolicy::Required
    } else {
        pol
    }
}

fn has_staged_matching(staged: Option<&BTreeSet<RepoPath>>, pred: fn(&RepoPath) -> bool) -> bool {
    staged.is_none_or(|set| set.iter().any(pred))
}

fn is_go_trigger(p: &RepoPath) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("go"))
        || matches!(
            p.file_name(),
            "go.mod" | "go.work" | ".golangci.yml" | ".golangci.yaml"
        )
}

fn is_kotlin_trigger(p: &RepoPath) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("kt") || e.eq_ignore_ascii_case("kts"))
        || matches!(p.file_name(), "detekt.yml" | "detekt.yaml" | "build.gradle")
}

fn is_csharp_trigger(p: &RepoPath) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cs") || e.eq_ignore_ascii_case("csproj"))
        || matches!(p.file_name(), "Directory.Build.props" | ".editorconfig")
}

fn run_cmd(bin: &Path, root: &Path, args: &[&str]) -> Result<std::process::Output, EngineError> {
    Command::new(bin)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(EngineError::Io)
}

#[derive(Clone, Copy)]
struct ToolSpec {
    bin: &'static str,
    id: &'static str,
    hint: &'static str,
}

fn resolve_tool(
    pol: ScannerPolicy,
    strict: bool,
    spec: ToolSpec,
    out: &mut Vec<Diagnostic>,
) -> Result<Option<std::path::PathBuf>, EngineError> {
    let eff = effective(pol, strict);
    if eff == ScannerPolicy::Off {
        return Ok(None);
    }
    let Some(bin) = resolve_system_binary(spec.bin) else {
        if eff == ScannerPolicy::Required {
            out.push(missing_tool_diagnostic(spec.id, spec.hint)?);
        }
        return Ok(None);
    };
    Ok(Some(bin))
}

type ToolParser = fn(&str) -> Result<Vec<gf_core::ExternalToolFinding>, gf_parsers::ToolParseError>;

#[derive(Clone, Copy)]
struct ParsedToolSpec {
    spec: ToolSpec,
    args: &'static [&'static str],
    parser: ToolParser,
}

const GOVET_SPEC: ParsedToolSpec = ParsedToolSpec {
    spec: ToolSpec {
        bin: "go",
        id: "go-vet",
        hint: "Install Go (`go vet`).",
    },
    args: &["vet", "-json", "./..."],
    parser: parse_go_vet_json,
};

const KTLINT_SPEC: ParsedToolSpec = ParsedToolSpec {
    spec: ToolSpec {
        bin: "ktlint",
        id: "ktlint",
        hint: "Install `ktlint` CLI.",
    },
    args: &["--reporter=json"],
    parser: parse_ktlint_json,
};

const DOTNET_SPEC: ParsedToolSpec = ParsedToolSpec {
    spec: ToolSpec {
        bin: "dotnet",
        id: "dotnet-build",
        hint: "Install the .NET SDK (`dotnet`).",
    },
    args: &["build", "--no-restore", "-nologo", "-v:q"],
    parser: parse_msbuild_diagnostics,
};

fn run_parsed_tool(
    ctx: &AdapterCtx<'_>,
    pol: ScannerPolicy,
    cfg: ParsedToolSpec,
    out: &mut Vec<Diagnostic>,
) -> Result<(), EngineError> {
    let Some(bin) = resolve_tool(pol, ctx.options.strict_tools, cfg.spec, out)? else {
        return Ok(());
    };
    let res = run_cmd(&bin, ctx.root, cfg.args)?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&res.stdout),
        String::from_utf8_lossy(&res.stderr)
    );
    if let Ok(findings) = (cfg.parser)(&combined) {
        out.extend(convert_external_tool_findings(&findings));
    }
    Ok(())
}

fn run_golangci(ctx: &AdapterCtx<'_>, out: &mut Vec<Diagnostic>) -> Result<(), EngineError> {
    let tools = &ctx.config.tools;
    let spec = ToolSpec {
        bin: "golangci-lint",
        id: "golangci-lint",
        hint: "Install `golangci-lint` v2 via `mise` or Go.",
    };
    let Some(bin) = resolve_tool(tools.go.golangci_lint, ctx.options.strict_tools, spec, out)?
    else {
        return Ok(());
    };
    let v2 = run_cmd(&bin, ctx.root, &["run", "--output.json.path", "stdout"])?;
    let stdout = String::from_utf8_lossy(&v2.stdout);
    let raw = if stdout.trim_start().starts_with('{') {
        stdout.into_owned()
    } else {
        let v1 = run_cmd(&bin, ctx.root, &["run", "--out-format", "json"])?;
        String::from_utf8_lossy(&v1.stdout).into_owned()
    };
    if let Ok(findings) = parse_golangci_lint_json(&raw) {
        out.extend(convert_external_tool_findings(&findings));
    }
    Ok(())
}

fn run_detekt(ctx: &AdapterCtx<'_>, out: &mut Vec<Diagnostic>) -> Result<(), EngineError> {
    let tools = &ctx.config.tools;
    let spec = ToolSpec {
        bin: "detekt",
        id: "detekt",
        hint: "Install `detekt` CLI.",
    };
    let Some(bin) = resolve_tool(tools.kotlin.detekt, ctx.options.strict_tools, spec, out)? else {
        return Ok(());
    };
    let sarif_path = std::env::temp_dir().join("gf-detekt.sarif");
    let report_arg = format!("sarif:{}", sarif_path.display());
    run_cmd(&bin, ctx.root, &["--report", &report_arg]).ok();
    if let Ok(raw) = std::fs::read_to_string(&sarif_path) {
        std::fs::remove_file(&sarif_path).ok();
        let mapping = SarifMapping {
            target_rule_id: "GF082",
            fallback_path: "detekt.yml",
            remediation: "Resolve the `detekt` Kotlin static analysis finding at its root cause.",
        };
        if let Ok(diags) = ingest_sarif_json(&raw, ctx.root, mapping) {
            out.extend(diags);
        }
    }
    Ok(())
}

/// Runs active Go, Kotlin, and C# Tier 3 runners with 0 ms `--staged` short-circuiting.
pub fn run_compiled_language_adapters(
    ctx: &AdapterCtx<'_>,
) -> Result<Vec<Diagnostic>, EngineError> {
    let tools = &ctx.config.tools;
    let mut out = Vec::new();
    if ctx.context.languages.contains(&LanguageId::Go)
        && has_staged_matching(ctx.staged_filter, is_go_trigger)
    {
        run_golangci(ctx, &mut out)?;
        run_parsed_tool(ctx, tools.go.govet, GOVET_SPEC, &mut out)?;
    }
    if ctx.context.languages.contains(&LanguageId::Kotlin)
        && has_staged_matching(ctx.staged_filter, is_kotlin_trigger)
    {
        run_detekt(ctx, &mut out)?;
        run_parsed_tool(ctx, tools.kotlin.ktlint, KTLINT_SPEC, &mut out)?;
    }
    if ctx.context.languages.contains(&LanguageId::CSharp)
        && has_staged_matching(ctx.staged_filter, is_csharp_trigger)
    {
        run_parsed_tool(ctx, tools.csharp.dotnet_build, DOTNET_SPEC, &mut out)?;
    }
    Ok(out)
}
