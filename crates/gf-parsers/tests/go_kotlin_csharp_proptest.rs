//! Property-based tests (`proptest`) for Go, Kotlin, and C# extractors, configs,
//! tool output parsers, and exact line-number covariance (`+k` leading comments).

use gf_core::RepoPath;
use gf_parsers::{
    parse_go_vet_json, parse_golangci_lint_json, parse_ktlint_json, parse_msbuild_diagnostics,
    parse_source,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn compiled_extractors_and_parsers_never_panic_on_arbitrary_utf8(raw in "\\PC{0,180}") {
        for path_str in ["src/fuzz.go", "src/Fuzz.kt", "src/Fuzz.cs", "go.mod", ".golangci.yml", "detekt.yml", "App.csproj"] {
            if let Ok(rp) = RepoPath::new(path_str)
                && let Ok(doc) = parse_source(&rp, &raw)
            {
                prop_assert!(doc.effective_lines <= doc.total_lines);
            }
        }
        let ms = parse_msbuild_diagnostics(&raw);
        prop_assert!(ms.is_ok());
        let _gl_ok = parse_golangci_lint_json(&raw).is_ok();
        let _gv_ok = parse_go_vet_json(&raw).is_ok();
        let _kl_ok = parse_ktlint_json(&raw).is_ok();
    }

    #[test]
    fn line_numbers_covary_exactly_with_leading_comments(k in 0_u32..12_u32) {
        let prefix = "// pad\n".repeat(usize::try_from(k).unwrap_or(0));
        let go_src = format!("{prefix}package p\nfunc f() error {{\n    var e *int\n    return e\n}}\n");
        let kt_src = format!("{prefix}package p\nfun f(x: String?) = x!!.length\n");
        let cs_src = format!("{prefix}namespace P;\npublic class C {{ public async void M() {{ await T(); }} }}\n");

        if let (Ok(gp), Ok(kp), Ok(cp)) = (
            RepoPath::new("a.go"),
            RepoPath::new("A.kt"),
            RepoPath::new("A.cs"),
        ) && let (Ok(gd), Ok(kd), Ok(cd)) = (
            parse_source(&gp, &go_src),
            parse_source(&kp, &kt_src),
            parse_source(&cp, &cs_src),
        ) {
            prop_assert_eq!(gd.lang_rule_findings[0].span.start_line(), k + 4);
            prop_assert_eq!(kd.lang_rule_findings[0].span.start_line(), k + 2);
            prop_assert_eq!(cd.lang_rule_findings[0].span.start_line(), k + 2);
        }
    }
}

#[test]
fn proptest_suite_has_top_level_sanity_assertion() -> Result<(), Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new("a.go")?, "package main\nfunc main() {}\n")?;
    assert_eq!(doc.syntax_errors.len(), 0);
    Ok(())
}

#[test]
fn compiled_tier2_configs_and_tier3_tool_parsers_handle_all_formats()
-> Result<(), Box<dyn std::error::Error>> {
    use gf_core::{CompiledConfigIssue, ExternalToolKind, ToolConfigViolation};
    let gomod = parse_source(
        &RepoPath::new("go.mod")?,
        "module acme.io/x\nreplace a => ../local\n",
    )?;
    assert!(
        gomod.tool_config_findings.iter().any(|f| f.violation
            == ToolConfigViolation::Compiled(CompiledConfigIssue::GoModLocalReplace))
    );
    assert!(
        gomod.tool_config_findings.iter().any(|f| f.violation
            == ToolConfigViolation::Compiled(CompiledConfigIssue::GoModMissingVersion))
    );
    let gl_v2 = parse_source(
        &RepoPath::new(".golangci.yml")?,
        "version: \"2\"\nlinters:\n  default: none\nrun:\n  tests: false\n",
    )?;
    assert_eq!(gl_v2.tool_config_findings.len(), 2);
    let detekt = parse_source(
        &RepoPath::new("detekt.yml")?,
        "build:\n  maxIssues: 5\ncoroutines:\n  active: false\n",
    )?;
    assert_eq!(detekt.tool_config_findings.len(), 2);
    let csproj = parse_source(
        &RepoPath::new("App.csproj")?,
        "<Project><PropertyGroup><Nullable>disable</Nullable><NoWarn>CS8618</NoWarn></PropertyGroup></Project>",
    )?;
    assert_eq!(csproj.tool_config_findings.len(), 2);
    let golangci_out = parse_golangci_lint_json(
        r#"{"Issues":[{"FromLinter":"errcheck","Text":"unchecked","Pos":{"Filename":"a.go","Line":4,"Column":2}}]}"#,
    )?;
    assert_eq!(golangci_out[0].tool, ExternalToolKind::GolangciLint);
    let govet_out = parse_go_vet_json(
        r#"{"acme/pkg":{"printf":[{"posn":"a.go:9:3","message":"bad format"}]}}"#,
    )?;
    assert_eq!(govet_out[0].tool, ExternalToolKind::GoVet);
    let ktlint_out = parse_ktlint_json(
        r#"[{"file":"App.kt","errors":[{"line":2,"column":1,"message":"indent","rule":"indent"}]}]"#,
    )?;
    assert_eq!(ktlint_out[0].tool, ExternalToolKind::Ktlint);
    let msbuild_out = parse_msbuild_diagnostics(
        "src\\Svc.cs(42,13): error CS8602: Dereference of null. [C:\\repo\\App.csproj]\n",
    )?;
    assert_eq!(msbuild_out[0].tool, ExternalToolKind::DotnetBuild);
    assert_eq!(msbuild_out[0].path.as_str(), "src/Svc.cs");
    assert_eq!(msbuild_out[0].line, 42);
    Ok(())
}
