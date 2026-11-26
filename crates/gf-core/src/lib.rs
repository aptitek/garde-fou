//! Pure `#![no_std]` functional core types for **Garde-fou**.
//!
//! This crate defines algebraic domain types (`Span`, `RepoPath`, `UastDocument`,
//! `ProjectContext`, `DomainKind`, `DockerDocument`, `LcovSummary`, `Diagnostic`,
//! `Config`, and `Validation`) with zero I/O or `std` dependencies.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod academic_violations;
pub mod agent_fragments;
pub mod agent_templates;
pub mod archetype_policy;
pub mod boundaries;
pub mod clean_code;
pub mod commit_msg;
pub mod compiled_tools;
pub mod compiled_violations;
pub mod config;
pub mod context;
pub mod coverage;
pub mod db_procedural_violations;
pub mod diagnostic;
pub mod diagrams_i18n_violations;
pub mod docker;
pub mod domain_summaries;
pub mod domains;
pub mod external_tool_kind;
pub mod format_tools;
pub mod format_violations;
pub mod framework_id;
pub mod gitignore;
pub mod glob;
pub mod lang_rule_violation;
pub mod language_meta;
pub mod lcov;
pub mod managed_blocks;
pub mod metrics;
pub mod mutation;
pub mod mutation_config_issues;
pub mod path;
pub mod policies;
pub mod project_settings;
pub mod py_ecosystems_violations;
pub mod python_violations;
pub mod shell_ps_tools;
pub mod shell_ps_violations;
pub mod skills_catalog;
pub mod sql_tools;
pub mod sql_types;
pub mod sql_violations;
pub mod tool_config_violations;
pub mod tool_ingest;
pub mod toolchain_mern_violations;
pub mod tools_config;
pub mod ts_violations;
pub mod uast;
pub mod ui_i18n;
pub mod validation;
pub mod web_edge_summary;
pub mod web_edge_tools;
pub mod web_edge_violations;
pub mod yaml_tools;
pub mod yaml_violations;

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
pub use managed_blocks::{
    is_empty_generated_agents_header, strip_delimited_managed_block, strip_garde_lefthook_commands,
    strip_hash_managed_block, strip_html_managed_block,
};
pub use metrics::{
    CallableDecl, CallableKind, EffectSighting, EffectSightingKind, ExportDecl, ImportDecl,
    SecretFinding, SyntaxErrorNode, TypeDecl, TypeKind,
};
pub use mutation::{MutationConfig, MutationSummary, SurvivedMutant, truncate_replacement};
pub use path::{RepoPath, RepoPathError};
pub use project_settings::{GovernanceFlag, GovernanceFlags, ProjectSettingsState};
pub use validation::Validation;
pub use {
    academic_violations::{
        AcademicSummary, AcademicViolation, NotebookConfig, NotebookViolation, ObsidianViolation,
        QuartoViolation, WorkspaceRefItem,
    },
    archetype_policy::{
        ArchetypePolicy, ArchetypeSetting, DeadExportGate, ScriptTestGate, WorkspaceArchetype,
        classify_path_for_archetype,
    },
    config::{
        ArchitectureConfig, Config, CoverageConfig, ForbiddenRuleKind, ForbiddenRulesConfig,
        LimitsConfig, ProjectConfig, ScannerPolicy, ScannersConfig,
    },
    context::{FeatureId, FrameworkId, LanguageId, ProjectContext},
    diagnostic::{Diagnostic, Severity, Span, SpanError},
};
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
    compiled_tools::{
        CSharpToolsConfig, CompiledConfigIssue, GoToolsConfig, KotlinToolsConfig,
        is_compiled_tool_config_file,
    },
    compiled_violations::{
        CSharpViolation, CompiledLangViolation, CompiledModuleSummary, ConcreteTypeMethods,
        GoViolation, KotlinViolation, StructuralInterfaceDecl,
    },
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
        is_catalog_skill_id, parse_extra_skill_spec, select_contextual_skills,
    },
    sql_tools::{
        DrizzleConfigIssue, SqlToolConfigIssue, SqlToolsConfig, SqruffSqlfluffConfigIssue,
        SquawkConfigIssue, is_sql_tool_config_file,
    },
    sql_violations::{
        DestructiveDdlFinding, MigrationViolation, SqlConfig, SqlDialectKind, SqlDialectSetting,
        SqlDocument, SqlRole, SqlViolation,
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
    web_edge_summary::{WebEdgeSummary, WranglerSummary, is_web_edge_tool_config_file},
    web_edge_tools::{
        CloudflareViolation, DrizzleViolation, WebEdgeConfigIssue, WebEdgeViolation, WebToolsConfig,
    },
    web_edge_violations::{
        A11yViolation, AstroViolation, M3eViolation, PandaViolation, ReactRouterViolation,
        W3cViolation,
    },
    yaml_tools::{
        DclintConfigIssue, YamlCiConfigIssue, YamlToolsConfig, YamllintConfigIssue,
        ZizmorConfigIssue, is_yaml_tool_config_file,
    },
    yaml_violations::{
        ComposeViolation, WorkflowViolation, YamlFlavor, YamlViolation, classify_yaml_flavor,
    },
};
pub use {
    db_procedural_violations::{
        DbProceduralViolation, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation,
    },
    diagrams_i18n_violations::{
        DiagramI18nViolation, I18nViolation, MermaidViolation, PlantUmlViolation,
    },
    py_ecosystems_violations::{
        DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, PyEcosystemViolation,
        is_django_migration_path,
    },
    toolchain_mern_violations::{
        MernViolation, NodeViolation, ToolchainSummary, ToolchainViolation,
        is_toolchain_audited_file,
    },
    uast::{Language, UastDocument, UastKind, UastNode},
    ui_i18n::{UiLocale, UiLocalePolicy, ui_tr, ui_tr_fmt},
};
