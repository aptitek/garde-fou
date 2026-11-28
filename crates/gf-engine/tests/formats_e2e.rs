//! End-to-end engine tests for CSS (`Forme`), XML/SVG (`Asset` / `Meta`), TOML (`Meta`),
//! and Markdown/MDX (`Contenus`), including `ExternalAdapter` argv/policies and `node_modules/.bin`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Config, ExternalToolKind, FeatureId, LanguageId, ScannerPolicy};
use gf_engine::{ExternalAdapter, ScanScope, run_check};

static SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_ws(label: &str) -> Result<PathBuf, std::io::Error> {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-fmt-e2e-{label}-{}-{n}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn formats_discover_context_and_evaluate_clean_vs_violating_workspace()
-> Result<(), Box<dyn std::error::Error>> {
    let ws = temp_ws("formats-ws")?;
    fs::create_dir_all(ws.join("styles/tokens"))?;
    fs::create_dir_all(ws.join("assets"))?;
    fs::create_dir_all(ws.join("docs"))?;
    fs::write(
        ws.join("styles/tokens/palette.css"),
        ":root { --fg: #112233; --pad: 8px; }\n",
    )?;
    fs::write(
        ws.join("styles/card.css"),
        ".card { color: var(--fg); padding: var(--pad); }\n",
    )?;
    fs::write(
        ws.join("assets/logo.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><rect width=\"16\" height=\"16\" fill=\"currentColor\"/></svg>\n",
    )?;
    fs::write(
        ws.join("docs/guide.md"),
        "# Guide\n\n![Logo](../assets/logo.svg)\n\nSee [Guide](#guide).\n",
    )?;
    fs::write(ws.join("mise.toml"), "[tools]\njq = \"1.7.1\"\n")?;

    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    cfg.agents.sync_policy = ScannerPolicy::Off;
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;
    cfg.tools.css.stylelint = ScannerPolicy::Off;
    cfg.tools.markdown.rumdl = ScannerPolicy::Off;
    cfg.tools.toml.taplo = ScannerPolicy::Off;
    cfg.tools.xml.xmllint = ScannerPolicy::Off;

    let report = run_check(&ws, ScanScope::All, &cfg)?;
    assert!(report.context.languages.contains(&LanguageId::Css));
    assert!(report.context.languages.contains(&LanguageId::Xml));
    assert!(report.context.languages.contains(&LanguageId::Toml));
    assert!(report.context.languages.contains(&LanguageId::Markdown));
    assert!(report.context.features.contains(&FeatureId::Ui));
    assert!(report.context.features.contains(&FeatureId::Content));
    assert_eq!(report.diagnostics, Vec::new());

    fs::write(ws.join("taplo.toml"), "exclude = [\"**/*.toml\"]\n")?;
    let weakened = run_check(&ws, ScanScope::All, &cfg)?;
    assert_eq!(weakened.diagnostics.len(), 1);
    assert_eq!(weakened.diagnostics[0].rule_id, "GF081");

    fs::remove_dir_all(&ws)?;
    Ok(())
}

struct AdapterExpectation {
    adapter: ExternalAdapter,
    kind: ExternalToolKind,
    expected_policy: ScannerPolicy,
    reads_stderr: bool,
}

impl AdapterExpectation {
    const fn new(
        adapter: ExternalAdapter,
        kind: ExternalToolKind,
        expected_policy: ScannerPolicy,
        reads_stderr: bool,
    ) -> Self {
        Self {
            adapter,
            kind,
            expected_policy,
            reads_stderr,
        }
    }
}

const FORMAT_ADAPTER_CASES: [AdapterExpectation; 5] = [
    AdapterExpectation::new(
        ExternalAdapter::Stylelint,
        ExternalToolKind::Stylelint,
        ScannerPolicy::Required,
        false,
    ),
    AdapterExpectation::new(
        ExternalAdapter::Rumdl,
        ExternalToolKind::Rumdl,
        ScannerPolicy::Auto,
        false,
    ),
    AdapterExpectation::new(
        ExternalAdapter::Markdownlint,
        ExternalToolKind::Markdownlint,
        ScannerPolicy::Required,
        false,
    ),
    AdapterExpectation::new(
        ExternalAdapter::Taplo,
        ExternalToolKind::Taplo,
        ScannerPolicy::Off,
        true,
    ),
    AdapterExpectation::new(
        ExternalAdapter::Xmllint,
        ExternalToolKind::Xmllint,
        ScannerPolicy::Required,
        true,
    ),
];

#[test]
fn external_format_adapters_classify_kinds_policies_and_stderr_routing() {
    let mut cfg = Config::strict_defaults();
    cfg.tools.css.stylelint = ScannerPolicy::Required;
    cfg.tools.markdown.rumdl = ScannerPolicy::Auto;
    cfg.tools.markdown.markdownlint = ScannerPolicy::Required;
    cfg.tools.toml.taplo = ScannerPolicy::Off;
    cfg.tools.xml.xmllint = ScannerPolicy::Required;

    for case in FORMAT_ADAPTER_CASES {
        assert_eq!(case.adapter.tool_kind(), case.kind);
        assert_eq!(case.adapter.configured_policy(&cfg), case.expected_policy);
        assert_eq!(
            case.adapter.reads_stderr_when_stdout_empty(),
            case.reads_stderr
        );
        assert_ne!(case.adapter.install_hint(), "");
    }
    assert!(ExternalAdapter::Stylelint.is_typescript());
    assert!(ExternalAdapter::Markdownlint.is_typescript());
    assert!(!ExternalAdapter::Rumdl.is_typescript());
    assert!(!ExternalAdapter::Taplo.is_python());
    assert!(!ExternalAdapter::Xmllint.is_python());
}

#[test]
fn external_format_adapters_build_exact_argv() {
    assert_eq!(
        ExternalAdapter::Stylelint.build_args(&[String::from("styles/a.css")]),
        vec!["--formatter", "json", "--", "styles/a.css"]
    );
    assert_eq!(
        ExternalAdapter::Rumdl.build_args(&[String::from("docs/a.md")]),
        vec!["check", "--output-format", "json", "docs/a.md"]
    );
    assert_eq!(
        ExternalAdapter::Markdownlint.build_args(&[String::from("docs/a.md")]),
        vec!["--json", "docs/a.md"]
    );
    assert_eq!(
        ExternalAdapter::Taplo.build_args(&[String::from("Cargo.toml")]),
        vec!["check", "--colors", "never", "Cargo.toml"]
    );
    assert_eq!(
        ExternalAdapter::Xmllint.build_args(&[String::from("pom.xml")]),
        vec!["--noout", "--nonet", "--", "pom.xml"]
    );
}
