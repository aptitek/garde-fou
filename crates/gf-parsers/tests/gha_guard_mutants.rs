//! Mutation-killing boundary tests for GitHub Actions (`GHA-*`), `run:` AST bridges,
//! and Tier 2 YAML/CI tool configuration files (`GF081`).

use gf_core::{
    DclintConfigIssue, LangRuleViolation, RepoPath, ShellViolation, ToolConfigViolation,
    UastDocument, WorkflowViolation, YamlCiConfigIssue, YamllintConfigIssue,
};
use gf_parsers::parse_source;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse_doc(rel_path: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel_path)?, src)?)
}

fn count_gha(doc: &UastDocument, v: WorkflowViolation) -> usize {
    doc.lang_rule_findings
        .iter()
        .filter(|f| f.violation == LangRuleViolation::Workflow(v))
        .count()
}

fn has_shell(doc: &UastDocument, v: ShellViolation) -> bool {
    doc.lang_rule_findings
        .iter()
        .any(|f| f.violation == LangRuleViolation::Shell(v))
}

#[test]
fn gha_triggers_boundaries() -> TestResult {
    for on_hdr in [
        "on: pull_request_target\n",
        "on: [push, pull_request_target]\n",
    ] {
        let wf = format!(
            "{on_hdr}permissions: {{ contents: read }}\njobs:\n  b: {{ runs-on: ubuntu-latest, steps: [{{ uses: actions/checkout@v4, with: {{ ref: \"${{{{ github.event.pull_request.head.sha }}}}\" }} }}] }}\n"
        );
        let d = parse_doc(".github/workflows/pr.yml", &wf)?;
        assert_eq!(count_gha(&d, WorkflowViolation::PwnRequestCheckout), 1);
    }

    let safe_pr = parse_doc(
        ".github/workflows/safe_pr.yml",
        "on: [push, pull_request]\npermissions: read-all\njobs:\n  b: { runs-on: ubuntu-latest, steps: [{ uses: actions/checkout@v4, with: { ref: \"${{ github.event.pull_request.head.sha }}\" } }] }\n",
    )?;
    assert_eq!(
        count_gha(&safe_pr, WorkflowViolation::PwnRequestCheckout),
        0
    );
    assert_eq!(
        count_gha(&safe_pr, WorkflowViolation::PermissionsOrUnpinnedUses),
        0
    );
    Ok(())
}

#[test]
fn gha_permissions_boundaries() -> TestResult {
    let bad_2j = parse_doc(
        ".github/workflows/bad2j.yml",
        "on: { push: {} }\npermissions: { contents: write }\njobs:\n  j1: { runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n  j2: { runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n",
    )?;
    assert_eq!(
        count_gha(&bad_2j, WorkflowViolation::PermissionsOrUnpinnedUses),
        1
    );

    let wa = parse_doc(
        ".github/workflows/wa.yml",
        "on: { push: {} }\npermissions: write-all\njobs:\n  j: { runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n",
    )?;
    assert_eq!(
        count_gha(&wa, WorkflowViolation::PermissionsOrUnpinnedUses),
        1
    );

    let ok1 = parse_doc(
        ".github/workflows/r1.yml",
        "on: { push: {} }\npermissions: { contents: write }\njobs:\n  j: { runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n",
    )?;
    assert_eq!(
        count_gha(&ok1, WorkflowViolation::PermissionsOrUnpinnedUses),
        0
    );

    let ok2 = parse_doc(
        ".github/workflows/r2.yml",
        "on: { push: {} }\npermissions: { contents: write }\njobs:\n  j1: { permissions: { contents: read }, runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n  j2: { permissions: { contents: write }, runs-on: ubuntu-latest, steps: [{ run: echo ok }] }\n",
    )?;
    assert_eq!(
        count_gha(&ok2, WorkflowViolation::PermissionsOrUnpinnedUses),
        0
    );
    Ok(())
}

#[test]
fn gha_pinning_shell_defaults_and_composite_action_bridges() -> TestResult {
    let custom_shell = format!("bash --noprofile {{{}}}", 0);
    let e_shell = format!("bash -eo pipefail {{{}}}", 0);
    let complex = format!(
        "on: {{ issues: {{}} }}\npermissions: {{ issues: write }}\ndefaults: {{ run: {{ shell: sh }} }}\njobs:\n  p:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: .\\loc\n      - uses: \"@v4\"\n      - uses: \"o/r@\"\n      - run: |\n          cd /tmp/safe_under_sh_e\n          echo '${{{{\n            github.sha\n          }}}}'\n          [[ -f /x ]] && echo '${{{{ github[\"event\"][\"commits\"][0][\"message\"] }}}}'\n  cb:\n    runs-on: ubuntu-latest\n    defaults: {{ run: {{ shell: \"{custom_shell}\" }} }}\n    steps: [{{ run: \"cd /tmp/sub\" }}]\n  eb:\n    runs-on: ubuntu-latest\n    defaults: {{ run: {{ shell: \"{e_shell}\" }} }}\n    steps: [{{ run: \"cd /tmp/safe_under_bash_e\" }}]\n"
    );
    let dc = parse_doc(".github/workflows/c.yml", &complex)?;
    assert_eq!(
        count_gha(&dc, WorkflowViolation::PermissionsOrUnpinnedUses),
        2
    );
    let inj: Vec<_> = dc
        .lang_rule_findings
        .iter()
        .filter(|f| {
            f.violation == LangRuleViolation::Workflow(WorkflowViolation::TemplateInjection)
        })
        .collect();
    assert_eq!(inj.len(), 1);
    assert_eq!(inj[0].span.start_line(), 16);
    assert!(has_shell(&dc, ShellViolation::PosixBashism));
    let cds: Vec<_> = dc
        .lang_rule_findings
        .iter()
        .filter(|f| f.violation == LangRuleViolation::Shell(ShellViolation::UncheckedCd))
        .collect();
    assert_eq!(cds.len(), 1);
    assert_eq!(cds[0].span.start_line(), 20);
    Ok(())
}

#[test]
fn gha_double_quoted_windows_and_composite_action_bridges() -> TestResult {
    let dq = parse_doc(
        ".github/workflows/dq.yml",
        "on: { push: {} }\npermissions: { contents: read }\njobs:\n  q:\n    runs-on: ubuntu-latest\n    steps:\n      - run: \"eval $x\n          echo done\"\n",
    )?;
    let eval_f = dq
        .lang_rule_findings
        .iter()
        .find(|f| f.violation == LangRuleViolation::Shell(ShellViolation::ForbiddenEval))
        .ok_or("missing ForbiddenEval")?;
    assert_eq!(eval_f.span.start_line(), 7);
    assert_eq!(eval_f.span.end_line(), 7);

    let win_if = parse_doc(
        ".github/workflows/win.yml",
        "on: { push: {} }\npermissions: { contents: read }\njobs:\n  ws:\n    runs-on: ubuntu-latest\n    steps: [{ if: 'runner.os == \"Windows\"', run: \"Invoke-Expression $cmd\" }]\n",
    )?;
    assert!(
        win_if
            .lang_rule_findings
            .iter()
            .any(|f| matches!(f.violation, LangRuleViolation::PowerShell(_)))
    );

    let comp = parse_doc(
        ".github/actions/my/action.yml",
        "name: T\nruns:\n  using: composite\n  steps:\n    - { shell: pwsh, run: \"iex $x\" }\n",
    )?;
    assert!(
        comp.lang_rule_findings
            .iter()
            .any(|f| matches!(f.violation, LangRuleViolation::PowerShell(_)))
    );
    Ok(())
}

#[test]
fn tier2_config_enabled_vs_every_critical_rule_disabled() -> TestResult {
    let yl = parse_doc(
        ".yamllint.yml",
        "extends: default\nrules:\n  truthy: enable\n  key-duplicates: { level: error }\n",
    )?;
    assert_eq!(yl.tool_config_findings.len(), 0);

    let dc = parse_doc(
        ".dclintrc.yml",
        "exclude: [vendor/compose.yml]\nrules:\n  require-quotes-in-ports: 2\n  no-docker-sock: [2, always]\n  no-privileged-containers: { level: error }\n",
    )?;
    assert_eq!(dc.tool_config_findings.len(), 0);

    let ziz = parse_doc(
        "zizmor.yml",
        "rules:\n  unpinned-uses: { disable: false }\n",
    )?;
    assert_eq!(ziz.tool_config_findings.len(), 0);

    for rule in [
        "service-image-require-explicit-tag",
        "require-quotes-in-ports",
        "no-unbound-port-interfaces",
        "no-privileged-containers",
        "no-docker-sock",
    ] {
        let d = parse_doc(".dclintrc.yml", &format!("rules:\n  {rule}: 0\n"))?;
        assert!(d.tool_config_findings.iter().any(|f| {
            f.violation
                == ToolConfigViolation::YamlCi(YamlCiConfigIssue::Dclint(
                    DclintConfigIssue::CriticalRuleDisabled,
                ))
        }));
    }
    let yk = parse_doc(".yamllint", "rules:\n  key-duplicates: disable\n")?;
    assert!(yk.tool_config_findings.iter().any(|f| {
        f.violation
            == ToolConfigViolation::YamlCi(YamlCiConfigIssue::Yamllint(
                YamllintConfigIssue::KeyDuplicatesDisabled,
            ))
    }));
    Ok(())
}
