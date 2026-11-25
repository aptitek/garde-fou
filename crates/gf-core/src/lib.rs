//! Pure `#![no_std]` functional core types for **Garde-fou**.
//!
//! This crate defines algebraic domain types (`Span`, `RepoPath`, `UastDocument`,
//! `ProjectContext`, `DomainKind`, `DockerDocument`, `LcovSummary`, `Diagnostic`,
//! `Config`, and `Validation`) with zero I/O or `std` dependencies.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod agent_fragments;
pub mod agent_templates;
pub mod boundaries;
pub mod clean_code;
pub mod commit_msg;
pub mod config;
pub mod context;
pub mod coverage;
pub mod diagnostic;
pub mod docker;
pub mod domain_summaries;
pub mod domains;
pub mod format_tools;
pub mod format_violations;
pub mod framework_id;
pub mod gitignore;
pub mod glob;
pub mod lang_rule_violation;
pub mod language_meta;
pub mod lcov;
pub mod metrics;
pub mod mutation;
pub mod mutation_config_issues;
pub mod path;
pub mod policies;
pub mod python_violations;
pub mod shell_ps_tools;
pub mod shell_ps_violations;
pub mod skills_catalog;
pub mod tool_config_violations;
pub mod tool_ingest;
pub mod tools_config;
pub mod ts_violations;
pub mod uast;
pub mod validation;
pub mod yaml_tools;
pub mod yaml_violations;

pub use config::{
    ArchitectureConfig, Config, CoverageConfig, ForbiddenRuleKind, ForbiddenRulesConfig,
    LimitsConfig, ProjectConfig, ScannerPolicy, ScannersConfig,
};
pub use context::{FeatureId, FrameworkId, LanguageId, ProjectContext};
pub use diagnostic::{Diagnostic, Severity, Span, SpanError};
pub use docker::{
    DockerCommandForm, DockerDocument, DockerInstruction, DockerInstructionKind, DockerShellKind,
    DockerStageShell, FromStage,
};
pub use domain_summaries::{
    AssetDocument, AssetHazard, AssetHazardKind, ContenusDocument, ContenusIssue,
    ContenusIssueKind, FormeDocument, FormeFinding, FormeFindingKind, MarkdownLinkTarget,
    StyleLeakFinding, StyleLeakKind, TexteDocument, TexteIssue, TomlManifestSummary,
    UiCheckFinding, UiCheckKind,
};
pub use domains::{
    DEFAULT_MAX_ASSET_BYTES, DomainKind, DomainLayoutConfig, classify_path, is_asset_extension,
    is_binary_asset_extension, is_forme_token_path, is_root_governance_doc,
};
pub use gitignore::{
    MANAGED_END, MANAGED_START, compose_dockerignore, compose_gitignore, merge_managed_block,
};
pub use glob::glob_matches;
pub use lcov::{FileCoverage, LcovSummary, compute_crap_score, parse_lcov};
pub use metrics::{
    CallableDecl, CallableKind, EffectSighting, EffectSightingKind, ExportDecl, ImportDecl,
    SecretFinding, SyntaxErrorNode, TypeDecl, TypeKind,
};
pub use mutation::{MutationConfig, MutationSummary, SurvivedMutant, truncate_replacement};
pub use path::{RepoPath, RepoPathError};
pub use uast::{Language, UastDocument, UastKind, UastNode};
pub use validation::Validation;
pub use {
    agent_fragments::{
        AGENTS_MANAGED_END, AGENTS_MANAGED_START, compose_agents_managed_block,
        demote_headings_outside_fences, extract_agents_managed_block, merge_agents_md,
    },
    agent_templates::EMBEDDED_GARDE_FOU_SKILL,
    boundaries::{BoundaryRule, IntegrityConfig, LockStatus, default_atomic_design_boundaries},
    clean_code::{
        BypassFinding, CleanCodeConfig, ContractDecl, ContractImpl, KissYagniFinding,
        KissYagniKind, TddConfig, TddParityGate, TestCaseSummary, YagniRule,
    },
    commit_msg::{CommitConfig, validate_commit_message},
    format_tools::{
        CssToolsConfig, FormatConfigIssue, MarkdownConfigIssue, MarkdownToolsConfig,
        StylelintConfigIssue, SvgoConfigIssue, TaploConfigIssue, TomlToolsConfig, XmlToolsConfig,
        is_format_tool_config_file,
    },
    format_violations::{CssViolation, MarkdownViolation, TomlViolation, XmlSvgViolation},
    lang_rule_violation::LangRuleViolation,
    language_meta::{
        TsconfigStrictState, TsconfigSummary, is_known_strict_preset, is_ts_declaration_path,
    },
    python_violations::{
        CoverageFailUnder, DebugArtifactKind, ExceptViolationKind, PythonViolation, UnsafeExecKind,
    },
    shell_ps_tools::{
        DockerToolsConfig, HadolintConfigIssue, PowerShellToolsConfig, PsScriptAnalyzerConfigIssue,
        ShellInfraConfigIssue, ShellToolsConfig, ShellcheckConfigIssue,
    },
    shell_ps_violations::{PowerShellViolation, ShellDialect, ShellViolation},
    skills_catalog::{
        AgenticSyncStatus, AgentsConfig, RemoteSkillSpec, ResolvedSkillSpec, SkillEntry,
        parse_extra_skill_spec, select_contextual_skills,
    },
    tool_config_violations::{
        MutantsConfigIssue, MypyConfigIssue, PytestCoverageIssue, RuffConfigIssue,
        StrykerConfigIssue, ToolConfigViolation, TsconfigIssue,
    },
    tool_ingest::{
        ExternalToolFinding, ExternalToolId, ExternalToolKind, LangRuleFinding, PythonToolsConfig,
        ToolConfigFinding, TypeScriptToolsConfig, is_audited_tool_config_file,
        is_tsconfig_filename,
    },
    tools_config::ToolsConfig,
    ts_violations::{AsyncHazardKind, ReactViolation, TsDebugKind, TypescriptViolation},
    yaml_tools::{
        DclintConfigIssue, YamlCiConfigIssue, YamlToolsConfig, YamllintConfigIssue,
        ZizmorConfigIssue, is_yaml_tool_config_file,
    },
    yaml_violations::{
        ComposeViolation, WorkflowViolation, YamlFlavor, YamlViolation, classify_yaml_flavor,
    },
};
