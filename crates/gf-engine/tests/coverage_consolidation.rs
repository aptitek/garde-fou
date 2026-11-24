//! Targeted unit & integration tests consolidating line coverage above 88% across
//! `config_fortress`, `config_loader`, `agents/{fetcher,skill_audit,verifier}`,
//! `import_resolver`, `anti_cheat`, and `kiss_contracts`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{
    AgenticSyncStatus, Config, ProjectContext, ResolvedSkillSpec, ScannerPolicy,
    parse_extra_skill_spec, validate_commit_message,
};
use gf_engine::agents::{
    fetcher::{SkillFetchOutcome, SkillsFetchMode, fetch_and_install_skill},
    skill_audit::extract_skill_summary,
};
use gf_engine::{
    CheckOptions, EngineError, ScanScope, load_workspace_config, run_check_with_options, run_lock,
    verify_agentic_state, verify_garde_lock,
};

static COV_SEQ: AtomicU64 = AtomicU64::new(0);

const FULL_MANIFEST_TOML: &str = "schema_version = 1\n\
[project]\nname = \"demo\"\nlanguages = [\"python\", \"typescript\"]\nfeatures = [\"ui\", \"tdd\"]\nframeworks = [\"react\"]\n\
[limits]\nmax_file_lines = 220\nmax_function_lines = 45\nmax_parameters = 3\n\
[domains]\nmax_asset_bytes = 2048\n\
[integrity]\nrequire_lock = true\n\
[[boundaries]]\nfrom = \"src/atoms/**\"\nallow = [\"src/atoms/**\"]\ndeny = [\"src/pages/**\"]\nreason = \"atoms stay pure\"\n\
[clean_code]\nmax_nested_ternaries = 0\nmax_property_chain = 3\nmax_bool_parameters = 1\nforbid_wildcard_exports = true\nforbid_todos = true\nforbid_single_impl_traits = true\n\
[tdd]\nforbid_assertion_free_tests = true\nrequire_companion = false\nrequire_stories = false\n\
[commit]\nallowed_types = [\"feat\", \"fix\"]\nallowed_scopes = [\"core\", \"engine\"]\nmax_header_len = 68\n\
[agents]\nsync_policy = \"required\"\nremote_skills = false\nextra_skills = [\"acme/repo@custom-skill\"]\ndisabled_skills = [\"ast-grep\"]\n\
[tools.python]\nruff = \"required\"\nmypy = \"off\"\npyright = \"auto\"\n\
[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n\
[coverage]\nmin_line_coverage = 88\nlcov_path = \"coverage/lcov.info\"\n\
[rules]\nforbid_unsafe = true\n\
[paths]\nignore_paths = [\"dist/**\"]\nentrypoints = [\"src/cli.py\"]\n\
[cache]\nenabled = false\ndirectory = \".custom-cache\"\n";

fn fresh_cov_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = COV_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-cov-{tag}-{pid}-{nanos}-{seq}"));
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
fn loads_full_garde_fou_manifest_sections_and_rejects_bad_policies()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_cov_dir("manifest")?;
    write_rel(&dir, "garde-fou.toml", FULL_MANIFEST_TOML)?;

    let loaded = load_workspace_config(&dir)?;
    assert!(!loaded.cache_enabled);
    assert_eq!(loaded.analyzer.limits.max_file_lines, 220);
    assert_eq!(loaded.analyzer.tools.python.ruff, ScannerPolicy::Required);
    assert_eq!(loaded.analyzer.boundaries.len(), 1);
    assert!(
        validate_commit_message(
            "feat(core,engine): add strict parser",
            &loaded.analyzer.commit
        )
        .is_valid()
    );
    assert!(
        !validate_commit_message("feat(unknown): bad scope", &loaded.analyzer.commit).is_valid()
    );

    let digest = run_lock(&dir)?;
    assert_ne!(digest, "");
    let lock_ok = verify_garde_lock(&dir, loaded.analyzer.integrity.require_lock)?;
    assert_eq!(lock_ok, gf_core::LockStatus::Verified);

    for bad_toml in [
        "schema_version = 2\n",
        "schema_version = 1\n[agents]\nsync_policy = \"invalid\"\n",
        "schema_version = 1\n[agents]\nextra_skills = [\"broken_spec\"]\n",
        "schema_version = 1\n[tools.python]\nruff = \"maybe\"\n",
        "schema_version = 1\n[scanners]\nsecrets = \"bogus\"\n",
    ] {
        write_rel(&dir, "garde-fou.toml", bad_toml)?;
        assert!(matches!(
            load_workspace_config(&dir),
            Err(EngineError::InvalidConfig(_))
        ));
    }

    assert!(parse_extra_skill_spec("bad").is_err());
    assert!(parse_extra_skill_spec("a/../b@skill").is_err());
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn exercises_fetcher_recursive_lookup_and_multiline_frontmatter()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_cov_dir("fetcher")?;
    let mirror = dir.join("mirror");
    let dest = dir.join("dest");
    write_rel(
        &mirror,
        "plugins/nested/deep/my-skill/SKILL.md",
        "---\nname: my-skill\ndescription: >\n  Deeply nested multiline skill summary for testing.\n---\n# My Skill\n\nBody.\n",
    )?;
    write_rel(
        &mirror,
        "plugins/nested/deep/my-skill/scripts/check.py",
        "def ok() -> int:\n    return 1\n",
    )?;

    let spec = parse_extra_skill_spec("acme/skills@my-skill")?;
    assert!(matches!(
        fetch_and_install_skill(&spec, &dest, &SkillsFetchMode::Offline),
        SkillFetchOutcome::Unavailable(_)
    ));
    let installed =
        fetch_and_install_skill(&spec, &dest, &SkillsFetchMode::FromDir(mirror.clone()));
    assert!(matches!(installed, SkillFetchOutcome::Installed(_)));
    assert!(dest.join("my-skill/scripts/check.py").is_file());

    let missing_spec = ResolvedSkillSpec {
        id: String::from("nonexistent"),
        ..spec
    };
    assert!(matches!(
        fetch_and_install_skill(&missing_spec, &dest, &SkillsFetchMode::FromDir(mirror)),
        SkillFetchOutcome::Unavailable(_)
    ));
    assert_eq!(
        extract_skill_summary("# Heading Only\n\nFirst prose sentence here.\n", "x"),
        "First prose sentence here."
    );
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn verifier_flags_missing_agents_md_and_missing_core_skill()
-> Result<(), Box<dyn std::error::Error>> {
    let proj = fresh_cov_dir("verifier")?;
    let mut cfg = Config::strict_defaults();
    cfg.agents.sync_policy = ScannerPolicy::Required;
    assert_eq!(
        verify_agentic_state(&proj, &ProjectContext::default(), &cfg)?,
        AgenticSyncStatus::MissingAgentsMd
    );
    write_rel(
        &proj,
        "AGENTS.md",
        "# AGENTS\n<!-- >>> garde-fou managed >>> -->\ndrifted\n<!-- <<< garde-fou managed <<< -->\n",
    )?;
    assert_eq!(
        verify_agentic_state(&proj, &ProjectContext::default(), &cfg)?,
        AgenticSyncStatus::MissingOrOutdatedCoreSkill
    );
    fs::remove_dir_all(&proj)?;
    Ok(())
}

#[test]
fn detects_python_relative_import_cycles_single_impl_protocols_and_bypass_comments()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = fresh_cov_dir("cycles-proto")?;
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[tools.python]\nruff = \"off\"\nmypy = \"off\"\npyright = \"off\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "src/pkg/alpha.py",
        "from typing import Protocol\nfrom .beta import helper_b\n\nclass RepoPort(Protocol):\n    def fetch(self) -> int: ...\n\nclass SqlRepo(RepoPort):\n    def fetch(self) -> int:\n        return helper_b()\n",
    )?;
    write_rel(
        &dir,
        "src/pkg/beta.py",
        "from .alpha import SqlRepo\n\ndef helper_b() -> int:\n    # ruff: noqa: F401\n    return 1\n",
    )?;
    write_rel(
        &dir,
        "scripts/deploy.sh",
        "#!/bin/sh\n# shellcheck disable=SC2086\necho ok\n",
    )?;
    write_rel(
        &dir,
        "src/web/a.ts",
        "import { b } from \"./sub\";\n// @ts-expect-error\nexport const a = b + 1;\n",
    )?;
    write_rel(
        &dir,
        "src/web/sub/index.ts",
        "import { a } from \"../a\";\n// @ts-expect-error: intentional coercion\nexport const b: number = a;\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    for rule in ["GF070", "GF071", "GF075"] {
        assert!(
            report.diagnostics.iter().any(|d| d.rule_id == rule),
            "missing {rule}"
        );
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}
