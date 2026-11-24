//! End-to-end engine tests for YAML 1.2, Docker Compose (`FeatureId::Docker` auto-activation),
//! GitHub Actions workflows, Tier 3 `ExternalAdapter` argv/parsers, and `--staged` short-circuiting.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Config, ExternalToolKind, FeatureId, LanguageId, ScannerPolicy};
use gf_engine::{ExternalAdapter, ScanScope, run_check};

static SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_ws(label: &str) -> Result<PathBuf, std::io::Error> {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-yaml-e2e-{label}-{}-{n}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn compose_auto_enables_docker_feature_and_evaluates_yaml_and_gha_end_to_end()
-> Result<(), Box<dyn std::error::Error>> {
    let ws = temp_ws("compose-gha")?;
    fs::create_dir_all(ws.join(".github/workflows"))?;
    fs::write(
        ws.join("compose.yaml"),
        "services:\n  web:\n    image: nginx:1.27-alpine\n    user: \"101:101\"\n    ports:\n      - \"8080:8080\"\n",
    )?;
    fs::write(
        ws.join(".github/workflows/ci.yml"),
        "name: ci\non: [push]\npermissions:\n  contents: read\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: printf 'ok\\n'\n",
    )?;

    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    cfg.agents.sync_policy = ScannerPolicy::Off;
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;
    cfg.tools.yaml.yamllint = ScannerPolicy::Off;
    cfg.tools.yaml.dclint = ScannerPolicy::Off;
    cfg.tools.yaml.actionlint = ScannerPolicy::Off;
    cfg.tools.yaml.zizmor = ScannerPolicy::Off;

    let report = run_check(&ws, ScanScope::All, &cfg)?;
    assert!(report.context.languages.contains(&LanguageId::Yaml));
    assert!(report.context.features.contains(&FeatureId::Docker));
    assert!(report.context.features.contains(&FeatureId::CiGithub));
    assert_eq!(report.diagnostics, Vec::new());

    fs::write(ws.join(".yamllint"), "extends: relaxed\n")?;
    let weakened = run_check(&ws, ScanScope::All, &cfg)?;
    assert_eq!(weakened.diagnostics.len(), 1);
    assert_eq!(weakened.diagnostics[0].rule_id, "GF081");

    fs::remove_dir_all(&ws)?;
    Ok(())
}

#[test]
fn external_yaml_adapters_classify_kinds_and_policies() {
    let mut cfg = Config::strict_defaults();
    cfg.tools.yaml.yamllint = ScannerPolicy::Required;
    cfg.tools.yaml.dclint = ScannerPolicy::Off;
    cfg.tools.yaml.actionlint = ScannerPolicy::Required;
    cfg.tools.yaml.zizmor = ScannerPolicy::Off;

    for (adapter, kind, expected_policy) in [
        (
            ExternalAdapter::Yamllint,
            ExternalToolKind::Yamllint,
            ScannerPolicy::Required,
        ),
        (
            ExternalAdapter::Dclint,
            ExternalToolKind::Dclint,
            ScannerPolicy::Off,
        ),
        (
            ExternalAdapter::Actionlint,
            ExternalToolKind::Actionlint,
            ScannerPolicy::Required,
        ),
        (
            ExternalAdapter::Zizmor,
            ExternalToolKind::Zizmor,
            ScannerPolicy::Off,
        ),
    ] {
        assert_eq!(adapter.tool_kind(), kind);
        assert_eq!(adapter.configured_policy(&cfg), expected_policy);
        assert_ne!(adapter.install_hint(), "");
    }
    assert!(ExternalAdapter::Yamllint.is_python());
    assert!(ExternalAdapter::Zizmor.is_python());
    assert!(ExternalAdapter::Dclint.is_typescript());
    assert!(!ExternalAdapter::Actionlint.is_python());
    assert!(!ExternalAdapter::Actionlint.is_typescript());
}

#[test]
fn external_yaml_adapters_build_exact_argv() {
    assert_eq!(
        ExternalAdapter::Yamllint.build_args(&[String::from("Taskfile.yml")]),
        vec!["-f", "parsable", "--", "Taskfile.yml"]
    );
    assert_eq!(
        ExternalAdapter::Yamllint.build_args(&[]),
        vec!["-f", "parsable", "--", "."]
    );
    assert_eq!(
        ExternalAdapter::Dclint.build_args(&[String::from("compose.yaml")]),
        vec!["-f", "json", "compose.yaml"]
    );
    assert_eq!(
        ExternalAdapter::Actionlint.build_args(&[String::from(".github/workflows/ci.yml")]),
        vec!["-format", "{{json .}}", ".github/workflows/ci.yml"]
    );
    assert_eq!(
        ExternalAdapter::Zizmor.build_args(&[String::from(".github/workflows/ci.yml")]),
        vec!["--format", "sarif", ".github/workflows/ci.yml"]
    );
}
