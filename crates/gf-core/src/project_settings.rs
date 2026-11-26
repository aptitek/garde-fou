//! Pure `#![no_std]` interactive project-settings model (`ProjectSettingsState`)
//! distinguishing disk auto-detected context from pinned `garde-fou.toml` overrides.

use alloc::collections::BTreeSet;

use crate::archetype_policy::{ArchetypeSetting, WorkspaceArchetype};
use crate::config::Config;
use crate::context::{FeatureId, FrameworkId, LanguageId, ProjectContext};
use crate::policies::ScannerPolicy;

/// Individual governance boolean toggle edited in `garde config` / Tab `[2]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GovernanceFlag {
    /// `[integrity] require_lock = true`.
    RequireLock = 0,
    /// `[agents] sync_policy = "required"` (unchecked -> `"auto"`).
    SyncRequired = 1,
    /// `[agents] remote_skills = true`.
    RemoteSkills = 2,
    /// `[clean_code] forbid_todos = true`.
    ForbidTodos = 3,
    /// `[tdd] forbid_assertion_free_tests = true`.
    RequireRealAssertions = 4,
}

impl GovernanceFlag {
    /// Ordered list of all 5 governance checkboxes.
    pub const ALL: [Self; 5] = [
        Self::RequireLock,
        Self::SyncRequired,
        Self::RemoteSkills,
        Self::ForbidTodos,
        Self::RequireRealAssertions,
    ];

    const fn mask(self) -> u8 {
        1_u8 << (self as u8)
    }

    /// Human-readable checkbox label displayed in the TUI configurator.
    #[must_use]
    pub const fn label(self) -> &'static str {
        const LABELS: [&str; 5] = [
            "Verrouiller garde.lock (require_lock)",
            "Synchronisation stricte AGENTS.md (sync_policy = required)",
            "Télécharger les skills distants (remote_skills)",
            "Interdire les TODO/FIXME (forbid_todos)",
            "Exiger de vraies assertions de test (forbid_assertion_free_tests)",
        ];
        LABELS[self as usize]
    }
}

/// Compact bitset encoding the 5 governance checkboxes without exceeding `max_struct_bools`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GovernanceFlags(u8);

impl GovernanceFlags {
    /// Returns `true` when `flag` is enabled.
    #[must_use]
    pub const fn is_set(self, flag: GovernanceFlag) -> bool {
        (self.0 & flag.mask()) != 0
    }

    /// Sets or clears `flag` according to `enabled`.
    pub const fn set(&mut self, flag: GovernanceFlag, enabled: bool) {
        if enabled {
            self.0 |= flag.mask();
        } else {
            self.0 &= !flag.mask();
        }
    }

    /// Flips `flag` between enabled and disabled.
    pub const fn toggle(&mut self, flag: GovernanceFlag) {
        self.0 ^= flag.mask();
    }

    /// Counts how many governance flags differ between `self` and `other`.
    #[must_use]
    pub const fn diff_count(self, other: Self) -> usize {
        (self.0 ^ other.0).count_ones() as usize
    }
}

/// Complete typed state edited by `garde config` and Tab `[2]` of `garde tui`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectSettingsState {
    /// Configured archetype (`Auto` or explicit override).
    pub archetype: ArchetypeSetting,
    /// Archetype inferred automatically from repository files on disk.
    pub detected_archetype: WorkspaceArchetype,
    /// Languages explicitly pinned in `garde-fou.toml`.
    pub languages: BTreeSet<LanguageId>,
    /// Languages auto-detected from workspace files on disk.
    pub detected_languages: BTreeSet<LanguageId>,
    /// Frameworks explicitly pinned in `garde-fou.toml`.
    pub frameworks: BTreeSet<FrameworkId>,
    /// Frameworks auto-detected on disk.
    pub detected_frameworks: BTreeSet<FrameworkId>,
    /// Features explicitly pinned in `garde-fou.toml`.
    pub features: BTreeSet<FeatureId>,
    /// Features auto-detected on disk.
    pub detected_features: BTreeSet<FeatureId>,
    /// Governance checkbox bitset.
    pub governance: GovernanceFlags,
}

impl ProjectSettingsState {
    /// Constructs a [`ProjectSettingsState`] from loaded `config` and disk-detected `detected`.
    #[must_use]
    pub fn from_config_and_detected(config: &Config, detected: &ProjectContext) -> Self {
        let mut governance = GovernanceFlags::default();
        governance.set(GovernanceFlag::RequireLock, config.integrity.require_lock);
        governance.set(
            GovernanceFlag::SyncRequired,
            config.agents.sync_policy == ScannerPolicy::Required,
        );
        governance.set(GovernanceFlag::RemoteSkills, config.agents.remote_skills);
        governance.set(GovernanceFlag::ForbidTodos, config.clean_code.forbid_todos);
        governance.set(
            GovernanceFlag::RequireRealAssertions,
            config.tdd.forbid_assertion_free_tests,
        );
        Self {
            archetype: config.project.archetype,
            detected_archetype: detected.archetype,
            languages: config.project.languages.clone(),
            detected_languages: detected.languages.clone(),
            frameworks: config.project.frameworks.clone(),
            detected_frameworks: detected.frameworks.clone(),
            features: config.project.features.clone(),
            detected_features: detected.features.clone(),
            governance,
        }
    }

    /// Counts the exact number of unsaved changes relative to `baseline`.
    #[must_use]
    pub fn diff_count(&self, baseline: &Self) -> usize {
        let arch_diff = usize::from(self.archetype != baseline.archetype);
        let lang_diff = self
            .languages
            .symmetric_difference(&baseline.languages)
            .count();
        let fw_diff = self
            .frameworks
            .symmetric_difference(&baseline.frameworks)
            .count();
        let feat_diff = self
            .features
            .symmetric_difference(&baseline.features)
            .count();
        let gov_diff = self.governance.diff_count(baseline.governance);
        arch_diff + lang_diff + fw_diff + feat_diff + gov_diff
    }

    /// Resolves the effective [`ProjectContext`] combining pinned selections and detected fallback.
    #[must_use]
    pub fn effective_context(&self) -> ProjectContext {
        let archetype = match self.archetype {
            ArchetypeSetting::Auto => self.detected_archetype,
            ArchetypeSetting::Explicit(a) => a,
        };
        let mut languages = self.detected_languages.clone();
        languages.extend(self.languages.iter().copied());
        let mut features = self.detected_features.clone();
        features.extend(self.features.iter().copied());
        let mut frameworks = self.detected_frameworks.clone();
        frameworks.extend(self.frameworks.iter().copied());
        ProjectContext {
            archetype,
            languages,
            features,
            frameworks,
            dockerignore_entries: None,
        }
    }
}
