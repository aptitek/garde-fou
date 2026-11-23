//! Integration tests for native secret detection & redaction (`GF040`), `SARIF` 2.1.0
//! ingestion (`GF041` Gitleaks, `GF042` OSV-Scanner), and `LCOV` coverage gates (`GF050`).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_engine::scanners::gitleaks::missing_tool_diagnostic;
use gf_engine::{
    CheckOptions, EngineError, SarifMapping, ScanScope, ingest_sarif_json, run_check_with_options,
};

static SEC_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_sec_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = SEC_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-sec-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn assert_dir_never_contains(
    dir: &Path,
    forbidden: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            assert_dir_never_contains(&path, forbidden)?;
        } else {
            let bytes = fs::read(&path)?;
            let text = String::from_utf8_lossy(&bytes);
            for secret in forbidden {
                assert!(
                    !text.contains(secret),
                    "raw secret leaked into cached file {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn gf040_detects_secrets_and_redacts_raw_values_everywhere()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_sec_dir("gf040")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("garde-fou.toml"),
        "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;

    let pat_tail = "Z9y8X7w6V5u4T3s2R1q0P9o8N7m6L5k4J3i2";
    let pat = format!("{}{pat_tail}", "ghp_");
    let aws_tail = "IOSFODNN7REALKEY";
    let aws = format!("{}{aws_tail}", "AKIA");
    let pem = format!("{}EC {}", "-----BEGIN ", "PRIVATE KEY-----");
    let raw_pw = "UltraS3cr3tProdValue42";

    let source = format!(
        "export const gh = \"{pat}\";\n\
         export const aws = \"{aws}\";\n\
         export const key = \"{pem}\";\n\
         export const client_secret = \"{raw_pw}\";\n"
    );
    fs::write(dir.join("src/leak.ts"), source)?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, false))?;
    let gf040: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF040")
        .collect();
    assert_eq!(gf040.len(), 4);

    for diag in &gf040 {
        for raw in [pat_tail, aws_tail, raw_pw] {
            assert!(!diag.message.contains(raw));
            assert!(!diag.remediation.contains(raw));
        }
    }
    assert_dir_never_contains(&dir.join(".garde-fou-cache"), &[pat_tail, aws_tail, raw_pw])?;

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn ingests_realistic_gitleaks_sarif_v2_1_0() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("/workspace/demo");
    let tok = format!("{}{}", "ghp_", "LeakInSarifMessage1234567890abcdef");
    let gitleaks_sarif = format!(
        "{{\"version\":\"2.1.0\",\"runs\":[{{\
         \"tool\":{{\"driver\":{{\"name\":\"gitleaks\",\"rules\":[{{\"id\":\"github-pat\",\"defaultConfiguration\":{{\"level\":\"error\"}}}}]}}}},\
         \"results\":[\
           {{\"ruleId\":\"github-pat\",\"message\":{{\"text\":\"Secret {tok} detected\"}},\"locations\":[{{\"physicalLocation\":{{\"artifactLocation\":{{\"uri\":\"src/auth.rs\"}},\"region\":{{\"startLine\":19}}}}}}]}},\
           {{\"ruleId\":\"github-pat\",\"message\":{{\"text\":\"Secret {tok} detected\"}},\"locations\":[{{\"physicalLocation\":{{\"artifactLocation\":{{\"uri\":\"./src/auth.rs\"}},\"region\":{{\"startLine\":19}}}}}}]}}\
         ]}}]}}"
    );

    let gl_diags = ingest_sarif_json(
        &gitleaks_sarif,
        root,
        SarifMapping {
            target_rule_id: "GF041",
            fallback_path: "garde-fou.toml",
            remediation: "Rotate secret.",
        },
    )?;
    assert_eq!(gl_diags.len(), 1);
    assert_eq!(gl_diags[0].rule_id, "GF041");
    assert_eq!(gl_diags[0].path.as_str(), "src/auth.rs");
    assert_eq!(gl_diags[0].span.start_line(), 19);
    assert!(!gl_diags[0].message.contains("LeakInSarifMessage"));
    assert!(gl_diags[0].message.contains("ghp_****"));
    Ok(())
}

#[test]
fn ingests_realistic_osv_sarif_v2_1_0() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("/workspace/demo");
    let osv_sarif = r#"{
      "version": "2.1.0",
      "runs": [{
        "tool": { "driver": { "name": "osv-scanner", "rules": [] } },
        "results": [{
          "ruleId": "RUSTSEC-2026-0001",
          "level": "warning",
          "message": { "text": "Package `foo@0.1.0` is vulnerable to RUSTSEC-2026-0001" },
          "locations": [{
            "physicalLocation": {
              "artifactLocation": { "uri": "file:///workspace/demo/Cargo.lock" },
              "region": { "startLine": 42 }
            }
          }]
        }]
      }]
    }"#;
    let osv_diags = ingest_sarif_json(
        osv_sarif,
        root,
        SarifMapping {
            target_rule_id: "GF042",
            fallback_path: "Cargo.lock",
            remediation: "Upgrade crate.",
        },
    )?;
    assert_eq!(osv_diags.len(), 1);
    assert_eq!(osv_diags[0].rule_id, "GF042");
    assert_eq!(osv_diags[0].path.as_str(), "Cargo.lock");
    assert_eq!(osv_diags[0].span.start_line(), 42);
    Ok(())
}

#[test]
fn gf050_blocks_72_percent_lcov_when_85_percent_is_required()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_sec_dir("gf050")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/lib.rs"),
        "/// Demo.\npub const fn ok() -> u32 { 1 }\n",
    )?;
    fs::write(
        dir.join("garde-fou.toml"),
        "schema_version = 1\n\
         [scanners]\nsecrets = \"off\"\nvulns = \"off\"\n\
         [coverage]\nmin_line_coverage = 85\nlcov_path = \"lcov.info\"\n",
    )?;

    let mut lcov_72 = String::from("TN:\nSF:src/lib.rs\n");
    for line in 1..=25_u32 {
        let hits = u32::from(line <= 18);
        let _ = writeln!(lcov_72, "DA:{line},{hits}");
    }
    lcov_72.push_str("end_of_record\n");
    fs::write(dir.join("lcov.info"), lcov_72)?;

    let failing = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(failing.diagnostics.len(), 1);
    assert_eq!(failing.diagnostics[0].rule_id, "GF050");
    assert_eq!(failing.diagnostics[0].measured, 72);
    assert_eq!(failing.diagnostics[0].threshold, 85);

    let mut lcov_92 = String::from("TN:\nSF:src/lib.rs\n");
    for line in 1..=25_u32 {
        let hits = u32::from(line <= 23);
        let _ = writeln!(lcov_92, "DA:{line},{hits}");
    }
    lcov_92.push_str("end_of_record\n");
    fs::write(dir.join("lcov.info"), lcov_92)?;

    let passing = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(passing.diagnostics, Vec::new());

    let missing_diag = missing_tool_diagnostic("gitleaks", "Install gitleaks")?;
    assert_eq!(missing_diag.rule_id, "GF043");

    fs::write(
        dir.join("garde-fou.toml"),
        "schema_version = 1\n[coverage]\nmin_line_coverage = 120\n",
    )?;
    assert!(matches!(
        run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true)),
        Err(EngineError::InvalidConfig(_))
    ));

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
