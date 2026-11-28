//! Unit tests for `gf-core` YAML, Docker Compose, and GitHub Actions sum types,
//! `YamlFlavor` path classification, Tier 2 config issues, and `[tools.yaml]` injectivity.

use std::collections::BTreeSet;

use gf_core::{
    ComposeViolation, Config, DclintConfigIssue, ExternalToolKind, LangRuleViolation, Language,
    LanguageId, RepoPath, ScannerPolicy, ToolConfigViolation, WorkflowViolation, YamlCiConfigIssue,
    YamlFlavor, YamlToolsConfig, YamlViolation, YamllintConfigIssue, ZizmorConfigIssue,
    classify_yaml_flavor, is_audited_tool_config_file,
};

#[test]
fn yaml_flavor_and_language_id_classification_are_exact() -> Result<(), Box<dyn std::error::Error>>
{
    assert_eq!(LanguageId::from_config_str("yaml"), Some(LanguageId::Yaml));
    assert_eq!(LanguageId::from_config_str("YML"), Some(LanguageId::Yaml));
    assert_eq!(LanguageId::Yaml.as_str(), "yaml");
    assert!(!LanguageId::Yaml.is_programming_language());
    assert!(LanguageId::Rust.is_programming_language());
    assert!(Language::Yaml.is_always_discovered());
    assert!(!Language::Yaml.is_code_analyzed());

    for (rel, expected) in [
        (
            "compose.yaml",
            YamlFlavor::DockerCompose { is_override: false },
        ),
        (
            "compose.prod.yaml",
            YamlFlavor::DockerCompose { is_override: false },
        ),
        (
            "infra/docker-compose.prod.yml",
            YamlFlavor::DockerCompose { is_override: false },
        ),
        (
            "docker-compose.override.yaml",
            YamlFlavor::DockerCompose { is_override: true },
        ),
        (".github/workflows/ci.yml", YamlFlavor::GithubWorkflow),
        (
            ".github/actions/build/action.yaml",
            YamlFlavor::GithubWorkflow,
        ),
        (".github/actions/build/helper.yaml", YamlFlavor::GenericYaml),
        ("tools/action.yml", YamlFlavor::GenericYaml),
        (".github/workflows/nested/sub.yml", YamlFlavor::GenericYaml),
        ("Taskfile.yml", YamlFlavor::GenericYaml),
        ("compose.txt", YamlFlavor::GenericYaml),
        (".github/workflows/README.md", YamlFlavor::GenericYaml),
    ] {
        assert_eq!(classify_yaml_flavor(&RepoPath::new(rel)?), expected);
    }
    Ok(())
}

fn assert_lrv_unique(
    lrv: LangRuleViolation,
    display_s: &str,
    codes: &mut BTreeSet<&'static str>,
    rems: &mut BTreeSet<&'static str>,
) {
    assert_eq!(display_s, lrv.code());
    assert!(!lrv.is_entrypoint_exempt());
    assert!(codes.insert(lrv.code()));
    assert!(rems.insert(lrv.remediation()));
}

#[test]
fn yaml_compose_and_workflow_violation_codes_and_conversions_are_injective() {
    let mut codes = BTreeSet::new();
    let mut rems = BTreeSet::new();
    for v in [
        YamlViolation::DuplicateKey,
        YamlViolation::TruthyAmbiguity,
        YamlViolation::ImplicitOctal,
        YamlViolation::SexagesimalValue,
        YamlViolation::AnchorBomb,
    ] {
        assert_lrv_unique(v.into(), &v.to_string(), &mut codes, &mut rems);
    }
    for v in [
        ComposeViolation::PrivilegedOrHostNamespace,
        ComposeViolation::DockerSocketMount,
        ComposeViolation::UnpinnedOrMissingImage,
        ComposeViolation::UnquotedPortMapping,
        ComposeViolation::PlaintextSecretInEnv,
        ComposeViolation::BrokenDependsOnGraph,
        ComposeViolation::OrphanOrUndeclaredResource,
    ] {
        assert_lrv_unique(v.into(), &v.to_string(), &mut codes, &mut rems);
    }
    for v in [
        WorkflowViolation::TemplateInjection,
        WorkflowViolation::PwnRequestCheckout,
        WorkflowViolation::PermissionsOrUnpinnedUses,
    ] {
        assert_lrv_unique(v.into(), &v.to_string(), &mut codes, &mut rems);
    }
}

#[test]
fn yaml_tool_config_predicates_are_exact() -> Result<(), Box<dyn std::error::Error>> {
    for cfg_file in [
        ".yamllint",
        ".yamllint.yml",
        ".yamllint.yaml",
        ".dclintrc",
        ".dclintrc.json",
        ".dclintrc.yml",
        ".dclintrc.yaml",
        "zizmor.yml",
        ".github/zizmor.yml",
    ] {
        assert!(is_audited_tool_config_file(&RepoPath::new(cfg_file)?));
    }
    Ok(())
}

struct YamlIssueCase {
    issue: YamlCiConfigIssue,
    tool: ExternalToolKind,
    key: &'static str,
}

const fn yaml_case(
    issue: YamlCiConfigIssue,
    tool: ExternalToolKind,
    key: &'static str,
) -> YamlIssueCase {
    YamlIssueCase { issue, tool, key }
}

#[test]
fn yaml_tool_config_issues_map_to_tools_and_keys() {
    for c in [
        yaml_case(
            YamlCiConfigIssue::Yamllint(YamllintConfigIssue::ExtendsRelaxed),
            ExternalToolKind::Yamllint,
            "extends",
        ),
        yaml_case(
            YamlCiConfigIssue::Yamllint(YamllintConfigIssue::TruthyDisabled),
            ExternalToolKind::Yamllint,
            "rules.truthy",
        ),
        yaml_case(
            YamlCiConfigIssue::Yamllint(YamllintConfigIssue::KeyDuplicatesDisabled),
            ExternalToolKind::Yamllint,
            "rules.key-duplicates",
        ),
        yaml_case(
            YamlCiConfigIssue::Dclint(DclintConfigIssue::CriticalRuleDisabled),
            ExternalToolKind::Dclint,
            "rules",
        ),
        yaml_case(
            YamlCiConfigIssue::Dclint(DclintConfigIssue::WildcardExcluded),
            ExternalToolKind::Dclint,
            "exclude",
        ),
        yaml_case(
            YamlCiConfigIssue::Zizmor(ZizmorConfigIssue::CriticalRuleDisabled),
            ExternalToolKind::Zizmor,
            "rules",
        ),
    ] {
        let tcv = ToolConfigViolation::YamlCi(c.issue);
        assert_eq!((tcv.tool(), tcv.key()), (c.tool, c.key));
        assert!(tcv.remediation().contains(c.tool.as_str()));
    }
}

#[test]
fn yaml_tools_config_canonical_string_is_injective() {
    assert_eq!(YamlToolsConfig::default(), YamlToolsConfig::defaults());
    let base = Config::strict_defaults();
    let mut seen = BTreeSet::from([base.canonical_string()]);
    for m in [
        |y: &mut YamlToolsConfig| y.yamllint = ScannerPolicy::Off,
        |y: &mut YamlToolsConfig| y.dclint = ScannerPolicy::Required,
        |y: &mut YamlToolsConfig| y.actionlint = ScannerPolicy::Off,
        |y: &mut YamlToolsConfig| y.zizmor = ScannerPolicy::Required,
    ] {
        let mut c = base.clone();
        m(&mut c.tools.yaml);
        assert!(seen.insert(c.canonical_string()));
    }
}
