//! Unit tests for `shell_ps_violations.rs`, `shell_ps_tools.rs`, `framework_id.rs`,
//! and enriched `DockerDocument` / `LanguageId` / `Language` invariants in `gf-core`.

use gf_core::{
    Config, DockerDocument, DockerShellKind, DockerStageShell, ExternalToolKind, FrameworkId,
    FromStage, HadolintConfigIssue, LangRuleFinding, LangRuleViolation, Language, LanguageId,
    PowerShellViolation, PsScriptAnalyzerConfigIssue, RepoPath, ScannerPolicy, ShellDialect,
    ShellInfraConfigIssue, ShellViolation, ShellcheckConfigIssue, Span, ToolConfigFinding,
    ToolConfigViolation, is_audited_tool_config_file,
};

type Res = Result<(), Box<dyn std::error::Error>>;

#[test]
fn shell_and_powershell_violations_have_unique_codes_and_remediations() {
    assert_eq!(ShellDialect::PosixSh.as_str(), "sh");
    assert_eq!(ShellDialect::Bash.as_str(), "bash");
    assert_eq!(format!("{}", ShellDialect::PosixSh), "sh");
    assert_eq!(format!("{}", ShellDialect::Bash), "bash");

    let expected_sh = [
        "SH-STRICT-MODE",
        "SH-SC2086",
        "SH-SC2046",
        "SH-SC2155",
        "SH-SC2115",
        "SH-SC2164",
        "SH-SC3000",
        "SH-NO-EVAL",
    ];
    for (v, expected) in ShellViolation::ALL.into_iter().zip(expected_sh) {
        assert_eq!(v.code(), expected);
        assert!(v.remediation().len() >= 25);
        let wrapped = LangRuleViolation::from(v);
        assert_eq!(wrapped.code(), expected);
        assert_eq!(wrapped.remediation(), v.remediation());
        assert!(!wrapped.is_entrypoint_exempt());
        let finding = LangRuleFinding::new(v, Span::FIRST, "msg");
        assert_eq!(finding.code(), expected);
    }

    let expected_ps = [
        "PS-STRICT-HEADER",
        "PS-NULL-LEFT",
        "PS-NO-IEX",
        "PS-PLAIN-SECRET-PARAM",
        "PS-EMPTY-CATCH",
        "PS-GLOBAL-VAR",
    ];
    for (v, expected) in PowerShellViolation::ALL.into_iter().zip(expected_ps) {
        assert_eq!(v.code(), expected);
        assert!(v.remediation().len() >= 25);
        let wrapped = LangRuleViolation::from(v);
        assert_eq!(wrapped.code(), expected);
        assert_eq!(wrapped.remediation(), v.remediation());
        assert!(!wrapped.is_entrypoint_exempt());
    }
}

#[test]
fn tier2_shell_infra_config_issues_match_specs() {
    let cases = [
        (
            ShellInfraConfigIssue::Shellcheck(ShellcheckConfigIssue::DisableAll),
            ExternalToolKind::Shellcheck,
            "disable",
        ),
        (
            ShellInfraConfigIssue::Shellcheck(ShellcheckConfigIssue::GlobalRangeDisable),
            ExternalToolKind::Shellcheck,
            "disable",
        ),
        (
            ShellInfraConfigIssue::Hadolint(HadolintConfigIssue::CriticalRuleIgnored),
            ExternalToolKind::Hadolint,
            "ignored",
        ),
        (
            ShellInfraConfigIssue::Hadolint(HadolintConfigIssue::WildcardTrustedRegistry),
            ExternalToolKind::Hadolint,
            "trustedRegistries",
        ),
        (
            ShellInfraConfigIssue::PsScriptAnalyzer(
                PsScriptAnalyzerConfigIssue::WildcardExcludeRules,
            ),
            ExternalToolKind::PsScriptAnalyzer,
            "ExcludeRules",
        ),
        (
            ShellInfraConfigIssue::PsScriptAnalyzer(
                PsScriptAnalyzerConfigIssue::CriticalSecurityRuleExcluded,
            ),
            ExternalToolKind::PsScriptAnalyzer,
            "ExcludeRules",
        ),
    ];
    for (issue, expected_tool, expected_key) in cases {
        let v = ToolConfigViolation::ShellInfra(issue);
        let f = ToolConfigFinding {
            violation: v,
            span: Span::FIRST,
            message: String::from("weakened"),
        };
        assert_eq!(f.tool(), expected_tool);
        assert_eq!(f.key(), expected_key);
        assert!(f.remediation().len() >= 25);
        assert_eq!(format!("{v}"), format!("{expected_tool}:{expected_key}"));
    }
}

#[test]
fn audited_shell_docker_and_psd1_filenames_are_recognized() -> Res {
    for audited in [
        ".shellcheckrc",
        "shellcheckrc",
        ".hadolint.yaml",
        ".hadolint.yml",
        "hadolint.yaml",
        "hadolint.yml",
        "PSScriptAnalyzerSettings.psd1",
    ] {
        assert!(is_audited_tool_config_file(&RepoPath::new(audited)?));
    }
    Ok(())
}

#[test]
fn language_ids_tools_config_and_docker_active_shell_invariants() {
    for (alias, expected) in [
        ("shell", LanguageId::Shell),
        ("sh", LanguageId::Shell),
        ("bash", LanguageId::Shell),
        ("powershell", LanguageId::PowerShell),
        ("pwsh", LanguageId::PowerShell),
        ("ps1", LanguageId::PowerShell),
    ] {
        assert_eq!(LanguageId::from_config_str(alias), Some(expected));
    }
    assert_eq!(LanguageId::Shell.as_str(), "shell");
    assert_eq!(LanguageId::PowerShell.as_str(), "powershell");
    assert!(Language::Shell.is_code_analyzed() && Language::Shell.is_always_discovered());
    assert!(Language::PowerShell.is_code_analyzed() && Language::PowerShell.is_always_discovered());
    assert_eq!(
        FrameworkId::from_config_str("cargo"),
        Some(FrameworkId::CargoWorkspace)
    );

    let mut cfg = Config::strict_defaults();
    let base_canon = cfg.canonical_string();
    assert!(base_canon.starts_with("v17|"));
    cfg.tools.shell.shellcheck = ScannerPolicy::Required;
    let c1 = cfg.canonical_string();
    assert_ne!(base_canon, c1);
    cfg.tools.powershell.psscriptanalyzer = ScannerPolicy::Auto;
    let c2 = cfg.canonical_string();
    assert_ne!(c1, c2);
    cfg.tools.docker.hadolint = ScannerPolicy::Off;
    assert_ne!(c2, cfg.canonical_string());

    let mut df = DockerDocument::default();
    assert_eq!(df.active_shell_kind(), DockerShellKind::PosixSh);
    df.stages.push(FromStage {
        image: String::from("mcr.microsoft.com/powershell"),
        tag: Some(String::from("7.4")),
        digest: None,
        alias: None,
        effective_user: None,
        effective_shell: DockerStageShell {
            kind: DockerShellKind::PowerShell,
            argv: vec![String::from("pwsh"), String::from("-Command")],
            pipefail_enabled: true,
        },
        span: Span::FIRST,
    });
    assert_eq!(df.active_shell_kind(), DockerShellKind::PowerShell);
}
