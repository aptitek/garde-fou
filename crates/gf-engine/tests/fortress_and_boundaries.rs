//! Integration tests for Step 7: Tarjan DAG cycles (`GF071`), Atomic Design `[[boundaries]]` (`GF072`),
//! Anti-Bypass & `garde.lock` (`GF070`), KISS/YAGNI (`GF073`–`GF075`), and TDD Fortress (`GF076`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{CommitConfig, Validation, validate_commit_message};
use gf_engine::{CheckOptions, ScanScope, run_check_with_options, run_lock};

static FORT_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_fort_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = FORT_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-fort-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_rel(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

#[test]
fn tarjan_detects_three_file_cycle_with_closed_chain() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_fort_dir("tarjan-3cycle")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/a.ts",
        "import { bVal } from \"./b\";\nexport const aVal = (): number => bVal() + 1;\n",
    )?;
    write_rel(
        &dir,
        "src/b.ts",
        "import { cVal } from \"./c\";\nexport const bVal = (): number => cVal() + 1;\n",
    )?;
    write_rel(
        &dir,
        "src/c.ts",
        "import { aVal } from \"./a\";\nexport const cVal = (): number => aVal() + 1;\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let cycle_msgs: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF071")
        .map(|d| d.message.as_str())
        .collect();
    assert_ne!(cycle_msgs, Vec::<&str>::new());
    assert!(
        cycle_msgs[0].contains("src/a.ts -> src/b.ts -> src/c.ts -> src/a.ts"),
        "unexpected cycle chain: {cycle_msgs:?}"
    );

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn atomic_design_blocks_atoms_importing_molecules_or_services()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_fort_dir("atomic-boundaries")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n\
         [project]\nfeatures = [\"ui\"]\n\
         [scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/components/molecules/SearchBar.tsx",
        "export function SearchBar() { return <input aria-label=\"q\" />; }\n",
    )?;
    write_rel(
        &dir,
        "src/services/api.ts",
        "export function fetchUsers(): number { return 1; }\n",
    )?;
    write_rel(
        &dir,
        "src/components/atoms/Button.tsx",
        "import { SearchBar } from \"../molecules/SearchBar\";\n\
         import { fetchUsers } from \"../../services/api\";\n\
         export function Button() {\n\
           void SearchBar;\n\
           void fetchUsers;\n\
           return <button type=\"button\" aria-label=\"ok\" />;\n\
         }\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let gf072_count = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF072")
        .count();
    assert!(
        gf072_count >= 2,
        "expected >= 2 GF072 violations, got {gf072_count}: {:#?}",
        report.diagnostics
    );

    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn populate_dirty_kiss_yagni_tdd_repo(dir: &Path) -> Result<(), std::io::Error> {
    write_rel(
        dir,
        "garde-fou.toml",
        "schema_version = 1\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(dir, "src/internal.ts", "export const INTERNAL_RATE = 42;\n")?;
    write_rel(
        dir,
        "src/smells.ts",
        "// eslint-disable-next-line no-console\n\
         export * from \"./internal\";\n\
         interface IUserRepo {\n\
           findById(id: string): string;\n\
         }\n\
         class SqlUserRepo implements IUserRepo {\n\
           findById(id: string): string { return id; }\n\
         }\n\
         export function pickLabel(a: boolean, b: boolean): string {\n\
           const repo = new SqlUserRepo();\n\
           void repo;\n\
           return a ? (b ? \"both\" : \"first\") : \"none\";\n\
         }\n",
    )?;
    write_rel(
        dir,
        "tests/smells.test.ts",
        "import { pickLabel } from \"../src/smells\";\n\
         test(\"works\", () => {\n\
           const x = pickLabel(true, false);\n\
           void x;\n\
         });\n",
    )
}

#[test]
fn dirty_code_triggers_gf070_gf073_gf074_gf075_and_gf076_together()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_fort_dir("kiss-yagni-tdd")?;
    populate_dirty_kiss_yagni_tdd_repo(&dir)?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let rules: BTreeSet<&str> = report
        .diagnostics
        .iter()
        .map(|d| d.rule_id.as_str())
        .collect();

    for expected in ["GF070", "GF073", "GF074", "GF075", "GF076"] {
        assert!(
            rules.contains(expected),
            "missing `{expected}` in {rules:?}\ndiagnostics: {:#?}",
            report.diagnostics
        );
    }

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn garde_lock_blocks_unauthorized_toml_tampering() -> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_fort_dir("lock-tamper")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n\
         [integrity]\nrequire_lock = true\n\
         [limits]\nmax_file_lines = 250\n\
         [scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/lib.rs",
        "/// Identity.\npub const fn id(x: u32) -> u32 {\n    x\n}\n",
    )?;

    let missing_lock = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        missing_lock
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "GF070")
    );

    run_lock(&dir)?;
    let clean = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        clean.is_clean(),
        "expected clean report, got {:#?}",
        clean.diagnostics
    );

    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n\
         [integrity]\nrequire_lock = true\n\
         [limits]\nmax_file_lines = 999\n\
         [scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    let tampered = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(
        tampered
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "GF070" && d.message.contains("garde.lock"))
    );

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn commit_msg_validator_accepts_conventional_and_rejects_vague() {
    let cfg = CommitConfig::strict_defaults();
    assert_eq!(
        validate_commit_message("feat(rules): add tarjan cycle detector\n", &cfg),
        Validation::Valid(())
    );
    assert!(matches!(
        validate_commit_message("wip: fix stuff\n", &cfg),
        Validation::Invalid(_)
    ));
}
