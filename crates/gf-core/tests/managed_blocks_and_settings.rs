//! Unit tests for `gf_core::managed_blocks` and `gf_core::project_settings`.

use gf_core::{
    ArchetypeSetting, Config, FeatureId, FrameworkId, GovernanceFlag, GovernanceFlags, LanguageId,
    ProjectContext, ProjectSettingsState, ScannerPolicy, WorkspaceArchetype,
    is_empty_generated_agents_header, merge_agents_md, merge_managed_block,
    strip_delimited_managed_block, strip_garde_lefthook_commands, strip_hash_managed_block,
    strip_html_managed_block,
};

#[test]
fn strip_delimited_and_html_and_hash_blocks_are_exact_inverses() {
    let raw = "before\n<!-- >>> garde-fou managed >>> -->\nbody\n<!-- <<< garde-fou managed <<< -->\nafter\n";
    assert_eq!(strip_html_managed_block(raw), "before\n\nafter\n");
    assert_eq!(
        strip_html_managed_block("no markers here\n"),
        "no markers here\n"
    );
    assert_eq!(
        strip_delimited_managed_block("a\nSTART\nx\nEND\n", "START", "END"),
        "a\n"
    );
    assert_eq!(
        strip_delimited_managed_block("START\nx\nEND\nb\n", "START", "END"),
        "b\n"
    );
    assert_eq!(
        strip_delimited_managed_block("START\nx\nEND", "START", "END"),
        ""
    );
    let merged_agents = merge_agents_md(Some("## Custom\n"), "rules");
    assert_eq!(strip_html_managed_block(&merged_agents), "## Custom\n");
    let block = "# >>> garde-fou managed >>>\n.garde-fou-cache/\n# <<< garde-fou managed <<<\n";
    let merged_gi = merge_managed_block(Some("*.local\n"), block);
    assert_eq!(strip_hash_managed_block(&merged_gi), "*.local\n");
}

#[test]
fn agents_header_detection_and_lefthook_stripping_are_exact() {
    assert!(is_empty_generated_agents_header("   \n\n"));
    let stub = "# AGENTS.md\n\n## Project-Specific Instructions\n\nAdd custom architecture notes and domain invariants here (preserved across `garde sync`).\n";
    assert!(is_empty_generated_agents_header(stub));
    assert!(!is_empty_generated_agents_header("## My Team Notes\n"));

    let garde_only = "pre-commit:\n  commands:\n    garde-check:\n      run: garde check --staged\ncommit-msg:\n  commands:\n    garde-commit:\n      run: garde commit-msg {1}\n";
    assert_eq!(strip_garde_lefthook_commands(garde_only), None);
    let garde_plain = "pre-commit:\n  commands:\n    g:\n      run: garde\n";
    assert_eq!(strip_garde_lefthook_commands(garde_plain), None);

    let mixed = "pre-commit:\n  commands:\n    fmt:\n      run: cargo fmt --check\n    garde-check:\n      run: garde check --staged\n";
    let cleaned = strip_garde_lefthook_commands(mixed);
    assert!(
        cleaned
            .as_deref()
            .is_some_and(|s| s.contains("cargo fmt --check") && !s.contains("garde check"))
    );
    let untouched = "pre-commit:\n  commands:\n    fmt:\n      run: cargo fmt --check\n";
    assert_eq!(
        strip_garde_lefthook_commands(untouched).as_deref(),
        Some(untouched)
    );
}

#[test]
fn governance_flags_and_project_settings_state_diff_exactly() {
    let mut flags = GovernanceFlags::default();
    assert!(GovernanceFlag::RequireLock.label().contains("garde.lock"));
    for flag in GovernanceFlag::ALL {
        assert!(!flags.is_set(flag));
        flags.set(flag, true);
        assert!(flags.is_set(flag));
        flags.toggle(flag);
        assert!(!flags.is_set(flag));
    }
    let mut f2 = GovernanceFlags::default();
    f2.set(GovernanceFlag::RequireLock, true);
    f2.set(GovernanceFlag::ForbidTodos, true);
    assert_eq!(flags.diff_count(f2), 2);
    f2.set(GovernanceFlag::RequireLock, false);
    assert!(!f2.is_set(GovernanceFlag::RequireLock));
    assert!(f2.is_set(GovernanceFlag::ForbidTodos));
    assert_eq!(flags.diff_count(f2), 1);

    let mut cfg = Config::strict_defaults();
    cfg.agents.sync_policy = ScannerPolicy::Required;
    let mut detected = ProjectContext {
        archetype: WorkspaceArchetype::Software,
        ..ProjectContext::default()
    };
    detected.languages.insert(LanguageId::Rust);
    detected.frameworks.insert(FrameworkId::CargoWorkspace);
    detected.features.insert(FeatureId::Docker);

    let base = ProjectSettingsState::from_config_and_detected(&cfg, &detected);
    assert_eq!(base.diff_count(&base), 0);
    assert!(base.governance.is_set(GovernanceFlag::SyncRequired));
    assert_eq!(
        base.effective_context().archetype,
        WorkspaceArchetype::Software
    );

    let mut draft = base.clone();
    draft.archetype = ArchetypeSetting::Explicit(WorkspaceArchetype::Infra);
    draft.languages.insert(LanguageId::Python);
    draft.frameworks.insert(FrameworkId::DockerCompose);
    draft.features.insert(FeatureId::Tdd);
    draft.governance.toggle(GovernanceFlag::RemoteSkills);
    assert_eq!(draft.diff_count(&base), 5);
    assert_eq!(
        draft.effective_context().archetype,
        WorkspaceArchetype::Infra
    );
}
