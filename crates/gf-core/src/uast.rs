//! Unified Abstract Syntax Tree (`UAST`) hub representations (`v9` schema).

use alloc::{collections::BTreeSet, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::clean_code::{
    BypassFinding, ContractDecl, ContractImpl, KissYagniFinding, TestCaseSummary,
};
use crate::diagnostic::{Span, SpanError};
use crate::domain_summaries::{
    AcademicSummary, AssetDocument, ContenusDocument, FormeDocument, StyleLeakFinding,
    TexteDocument, TomlManifestSummary, UiCheckFinding,
};
pub use crate::language_meta::Language;
use crate::metrics::{
    CallableDecl, EffectSighting, ExportDecl, ImportDecl, SecretFinding, SyntaxErrorNode, TypeDecl,
};
use crate::path::RepoPath;
use crate::policies::ForbiddenRuleKind;
use crate::tool_ingest::{LangRuleFinding, ToolConfigFinding};

/// Language-agnostic semantic node categories inside a [`UastDocument`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum UastKind {
    /// Root translation unit of a file.
    File,
    /// Function or method definition with normalized SRP metrics.
    Function {
        /// Identifier of the function.
        name: String,
        /// Number of declared parameters.
        param_count: u32,
        /// Computed cognitive complexity score.
        cognitive_complexity: u32,
    },
    /// Explicit call or construct flagged under `[rules]` (`GF010`–`GF011`).
    ForbiddenCall {
        /// Typed category enabling granular filtering via [`crate::ForbiddenRulesConfig`].
        rule: ForbiddenRuleKind,
        /// Exact symbol or macro detected (e.g. `".unwrap()"`, `"todo!"`, `"unsafe fn"`).
        symbol: String,
    },
}

/// Single node inside the Unified Abstract Syntax Tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UastNode {
    /// Semantic kind of this node.
    pub kind: UastKind,
    /// Source line span covered by this node.
    pub span: Span,
    /// Child nodes nested under this construct.
    pub children: Vec<Self>,
}

impl UastNode {
    /// Constructs a leaf `UAST` node with no children.
    #[must_use]
    pub const fn leaf(kind: UastKind, span: Span) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }
}

/// Normalized `UAST` document produced by `gf-parsers`, cached in `.garde-fou-cache/v9/`,
/// and analyzed locally and globally by `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UastDocument {
    /// Repository-relative file path.
    pub path: RepoPath,
    /// Detected language or domain format of the file.
    pub language: Language,
    /// Total number of lines in the raw source text.
    pub total_lines: u32,
    /// Total number of non-blank lines in the source text.
    pub effective_lines: u32,
    /// Syntax error or missing-token nodes detected in the file.
    pub syntax_errors: Vec<SyntaxErrorNode>,
    /// Module import declarations extracted from the file.
    pub imports: Vec<ImportDecl>,
    /// Public and crate-internal symbol export declarations extracted from the file.
    pub exports: Vec<ExportDecl>,
    /// Composite types and `impl` blocks extracted from the file.
    pub types: Vec<TypeDecl>,
    /// Functions, methods, and named closures extracted from the file.
    pub callables: Vec<CallableDecl>,
    /// Redacted native secret findings (`GF040`) extracted at parse time.
    pub secret_findings: Vec<SecretFinding>,
    /// Deterministic set of symbol identifiers referenced inside this file.
    pub referenced_symbols: BTreeSet<String>,
    /// Style or inline-asset leaks detected in `Code` (`GF060`, `GF064`).
    pub style_leaks: Vec<StyleLeakFinding>,
    /// UI `i18n` (`GF062`) and static `a11y` (`GF066`) findings in JSX/TSX.
    pub ui_checks: Vec<UiCheckFinding>,
    /// Translation keys passed to `t("...")` / `i18n.t("...")` (`GF062`).
    pub translation_keys_used: BTreeSet<String>,
    /// Local asset paths referenced in `Code`, `Forme`, or `Contenus` (`GF064`).
    pub asset_refs: BTreeSet<String>,
    /// Script paths or filenames mentioned in string literals (`GF065` test coverage).
    pub script_refs: BTreeSet<String>,
    /// Linter-bypass directives/attributes detected in CST (`GF070`).
    pub bypass_findings: Vec<BypassFinding>,
    /// Local KISS (`GF073`) and YAGNI (`GF074`) smells.
    pub kiss_yagni_findings: Vec<KissYagniFinding>,
    /// Behavioral contracts (`trait` / `interface` / `Protocol` with methods, `GF075`).
    pub contracts: Vec<ContractDecl>,
    /// Concrete `(contract, implementor)` bindings declared in this file (`GF075`).
    pub contract_impls: BTreeSet<ContractImpl>,
    /// Unit/integration test cases with assertion counts and quality flags (`GF076`).
    pub test_cases: Vec<TestCaseSummary>,
    /// Tier 1 native language-idiom & security AST findings (`GF080`).
    pub lang_rule_findings: Vec<LangRuleFinding>,
    /// Tier 2 static tool-configuration hardening findings (`GF081`).
    pub tool_config_findings: Vec<ToolConfigFinding>,
    /// Optional `[tool.coverage.report] fail_under` floor percentage (`GF081`).
    pub coverage_fail_under: Option<crate::python_violations::CoverageFailUnder>,
    /// Optional parsed `tsconfig*.json` summary for `extends` & `paths` resolution (`GF071`/`GF072`/`GF081`).
    #[serde(default)]
    pub tsconfig: Option<crate::language_meta::TsconfigSummary>,
    /// Architectural effect and silent error-swallowing sightings (`GF011`, `GF078`).
    pub effect_sightings: Vec<EffectSighting>,
    /// Optional structured `Dockerfile` representation.
    pub docker: Option<crate::docker::DockerDocument>,
    /// Optional `Forme` stylesheet summary (`GF061`).
    pub forme: Option<FormeDocument>,
    /// Optional `Texte` (`i18n`) dictionary summary (`GF062`).
    pub texte: Option<TexteDocument>,
    /// Optional `Contenus` Markdown/MDX summary (`GF063`).
    pub contenus: Option<ContenusDocument>,
    /// Optional `Asset` metadata & SVG security summary (`GF064`).
    pub asset: Option<AssetDocument>,
    /// Optional `Cargo.toml` workspace dependency summary (`GF080` `TOML-WORKSPACE-BYPASS`).
    #[serde(default)]
    pub toml_manifest: Option<TomlManifestSummary>,
    /// Optional SQL & database migration summary (`GF080` `MIG-*`).
    #[serde(default)]
    pub sql_doc: Option<crate::sql_violations::SqlDocument>,
    /// Optional Web, Cloudflare/Wrangler, Drizzle, and `Forme` motion summary (`GF080`–`GF081`).
    #[serde(default)]
    pub web_edge: Option<crate::web_edge_summary::WebEdgeSummary>,
    /// Optional Jupyter, Quarto, or Obsidian workspace-ref & cell summary (`GF080`).
    #[serde(default)]
    pub academic: Option<AcademicSummary>,
    /// Root `UAST` node (typically [`UastKind::File`]).
    pub root: UastNode,
}

impl UastDocument {
    /// Constructs an empty [`UastDocument`] with initialized line metrics and file span.
    ///
    /// # Errors
    ///
    /// Returns [`SpanError`] if constructing the root span fails.
    pub fn new(
        path: RepoPath,
        language: Language,
        total_lines: u32,
        effective_lines: u32,
    ) -> Result<Self, SpanError> {
        let root_span = Span::new(1, total_lines.max(1))?;
        Ok(Self {
            path,
            language,
            total_lines,
            effective_lines,
            syntax_errors: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            types: Vec::new(),
            callables: Vec::new(),
            secret_findings: Vec::new(),
            referenced_symbols: BTreeSet::new(),
            style_leaks: Vec::new(),
            ui_checks: Vec::new(),
            translation_keys_used: BTreeSet::new(),
            asset_refs: BTreeSet::new(),
            script_refs: BTreeSet::new(),
            bypass_findings: Vec::new(),
            kiss_yagni_findings: Vec::new(),
            contracts: Vec::new(),
            contract_impls: BTreeSet::new(),
            test_cases: Vec::new(),
            lang_rule_findings: Vec::new(),
            tool_config_findings: Vec::new(),
            coverage_fail_under: None,
            tsconfig: None,
            effect_sightings: Vec::new(),
            docker: None,
            forme: None,
            texte: None,
            contenus: None,
            asset: None,
            toml_manifest: None,
            sql_doc: None,
            web_edge: None,
            academic: None,
            root: UastNode::leaf(UastKind::File, root_span),
        })
    }
}
