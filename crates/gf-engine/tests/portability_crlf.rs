//! Cross-platform portability tests: Windows `\` paths, drive-letter rejection, Windows
//! `file:///C:/...` SARIF URIs, and `CRLF` (`\r\n`) vs `LF` (`\n`) bit-for-bit equivalence.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{
    Config, LanguageId, ProjectContext, RepoPath, RepoPathError, compose_gitignore,
    merge_managed_block, parse_lcov,
};
use gf_engine::{CheckOptions, SarifMapping, ScanScope, ingest_sarif_json, run_check_with_options};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

static PORT_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_port_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = PORT_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-port-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn normalizes_windows_backslashes_and_file_uris_while_rejecting_drive_roots()
-> Result<(), Box<dyn std::error::Error>> {
    let rel = RepoPath::new(r"crates\gf-core\src\lib.rs")?;
    assert_eq!(rel.as_str(), "crates/gf-core/src/lib.rs");
    assert_eq!(rel.file_name(), "lib.rs");
    assert_eq!(
        RepoPath::new(r"C:\Users\dev\secret.rs"),
        Err(RepoPathError::Absolute)
    );
    assert_eq!(
        RepoPath::new("d:/workspace/main.rs"),
        Err(RepoPathError::Absolute)
    );
    assert_eq!(
        RepoPath::new(r"src\..\..\etc\passwd"),
        Err(RepoPathError::ParentTraversal)
    );

    let win_sarif = r#"{
      "version": "2.1.0",
      "runs": [{
        "tool": { "driver": { "name": "osv-scanner", "rules": [] } },
        "results": [{
          "ruleId": "GHSA-0000-1111-2222",
          "level": "error",
          "message": { "text": "Vulnerable lockfile package" },
          "locations": [{
            "physicalLocation": {
              "artifactLocation": { "uri": "file:///c:/Users/Runner/Work/App/Cargo.lock" },
              "region": { "startLine": 12 }
            }
          }]
        }]
      }]
    }"#;
    let diags = ingest_sarif_json(
        win_sarif,
        Path::new(r"C:\Users\Runner\Work\App"),
        SarifMapping {
            target_rule_id: "GF042",
            fallback_path: "Cargo.lock",
            remediation: "Upgrade dependency.",
        },
    )?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].path.as_str(), "Cargo.lock");
    assert_eq!(diags[0].span.start_line(), 12);
    Ok(())
}

#[test]
fn dockerfile_continuations_and_heredocs_are_identical_under_crlf_and_lf()
-> Result<(), Box<dyn std::error::Error>> {
    let lf_dockerfile = "# syntax=docker/dockerfile:1\n\
        # escape=\\\n\
        FROM python:3.13-slim AS runtime\n\
        WORKDIR /app\n\
        RUN pip install \\\n\
            --no-cache-dir \\\n\
            fastapi\n\
        RUN <<EOF\n\
        useradd --system --uid 10001 appuser\n\
        EOF\n\
        USER appuser\n";
    let crlf_dockerfile = lf_dockerfile.replace('\n', "\r\n");

    let path = RepoPath::new("Dockerfile")?;
    let doc_lf = parse_source(&path, lf_dockerfile)?;
    let doc_crlf = parse_source(&path, &crlf_dockerfile)?;

    assert_eq!(doc_lf, doc_crlf);
    let cfg = Config::strict_defaults();
    assert_eq!(
        evaluate_document(&doc_lf, &cfg),
        evaluate_document(&doc_crlf, &cfg)
    );
    Ok(())
}

#[test]
fn lcov_and_gitignore_produce_identical_results_with_crlf_and_windows_paths() -> Result<(), String>
{
    let lcov_lf = "TN:\nSF:./src/core/lib.rs\nDA:1,4\nDA:2,0\nDA:3,1\nend_of_record\n";
    let lcov_crlf =
        "TN:\r\nSF:.\\src\\core\\lib.rs\r\nDA:1,4\r\nDA:2,0\r\nDA:3,1\r\nend_of_record\r\n";
    assert_eq!(parse_lcov(lcov_lf), parse_lcov(lcov_crlf));

    let mut ctx = ProjectContext::default();
    ctx.languages.insert(LanguageId::Rust);
    let managed = compose_gitignore(&ctx);

    let mut runner = TestRunner::new(ProptestConfig {
        cases: 64,
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let strat = prop::collection::vec("[a-zA-Z0-9_./-]{1,16}", 1..5);
    runner
        .run(&strat, |lines| {
            let lf_existing = format!("{}\n", lines.join("\n"));
            let crlf_existing = lf_existing.replace('\n', "\r\n");
            let merged_lf = merge_managed_block(Some(&lf_existing), &managed);
            let merged_crlf = merge_managed_block(Some(&crlf_existing), &managed);
            prop_assert_eq!(&merged_lf, &merged_crlf);
            prop_assert!(!merged_crlf.contains('\r'));
            Ok(())
        })
        .map_err(|e| e.to_string())
}

#[test]
fn workspace_check_and_blake3_fingerprint_match_across_crlf_and_lf()
-> Result<(), Box<dyn std::error::Error>> {
    let dir_lf = temp_port_dir("ws-lf")?;
    let dir_crlf = temp_port_dir("ws-crlf")?;
    let files = [
        (
            "garde-fou.toml",
            "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
        ),
        (
            "src/lib.rs",
            "/// Adds two integers.\npub const fn add(a: i32, b: i32) -> i32 {\n    if a > 0 {\n        a + b\n    } else {\n        b\n    }\n}\n",
        ),
        (
            "src/index.ts",
            "export function clampPositive(n: number): number {\n  return n > 0 ? n : 0;\n}\n",
        ),
        (
            "src/main.py",
            "def compute_ratio(num: int, den: int) -> int:\n    if den == 0:\n        return 0\n    return num // den\n",
        ),
    ];

    for (rel, body_lf) in files {
        let p_lf = dir_lf.join(rel);
        let p_crlf = dir_crlf.join(rel);
        if let Some(parent) = p_lf.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = p_crlf.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p_lf, body_lf)?;
        fs::write(p_crlf, body_lf.replace('\n', "\r\n"))?;
    }

    let rep_lf = run_check_with_options(&dir_lf, &CheckOptions::new(ScanScope::All, true))?;
    let rep_crlf = run_check_with_options(&dir_crlf, &CheckOptions::new(ScanScope::All, true))?;

    assert!(rep_lf.is_clean());
    assert_eq!(rep_lf.cache_fingerprint, rep_crlf.cache_fingerprint);
    assert_eq!(rep_lf.diagnostics, rep_crlf.diagnostics);

    let _ = fs::remove_dir_all(&dir_lf);
    let _ = fs::remove_dir_all(&dir_crlf);
    Ok(())
}
