//! Individual pure syntax (`GF000`), file (`GF001`, `GF007`), and callable (`GF002`–`GF006`)
//! metric rule evaluators, plus shared [`SourceLoc`] diagnostic constructors.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    CallableDecl, Config, Diagnostic, Language, LimitsConfig, RepoPath, Severity, Span,
    UastDocument, Validation,
};

/// Source location pairing a repository-relative [`RepoPath`] and 1-indexed [`Span`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLoc<'a> {
    /// Repository-relative file path.
    pub path: &'a RepoPath,
    /// 1-indexed inclusive source span.
    pub span: Span,
}

impl<'a> SourceLoc<'a> {
    /// Constructs a [`SourceLoc`] for `path` and `span`.
    #[must_use]
    pub const fn new(path: &'a RepoPath, span: Span) -> Self {
        Self { path, span }
    }
}

pub struct MetricErrorSpec<'a> {
    pub rule_id: &'static str,
    pub message: String,
    pub remediation: &'static str,
    pub measured: u32,
    pub threshold: u32,
    pub loc: SourceLoc<'a>,
}

pub fn build_metric_error(spec: MetricErrorSpec<'_>) -> Diagnostic {
    Diagnostic {
        rule_id: String::from(spec.rule_id),
        severity: Severity::Error,
        message: spec.message,
        remediation: String::from(spec.remediation),
        measured: spec.measured,
        threshold: spec.threshold,
        path: spec.loc.path.clone(),
        span: spec.loc.span,
    }
}

/// Shared constructor for boolean/structural rule violations (`measured = 1, threshold = 0`).
#[must_use]
pub fn rule_error(
    rule_id: &str,
    message: String,
    remediation: &str,
    loc: SourceLoc<'_>,
) -> Diagnostic {
    Diagnostic {
        rule_id: String::from(rule_id),
        severity: Severity::Error,
        message,
        remediation: String::from(remediation),
        measured: 1,
        threshold: 0,
        path: loc.path.clone(),
        span: loc.span,
    }
}

pub fn check_syntax_errors(doc: &UastDocument) -> Validation<(), Diagnostic> {
    let diags = doc
        .syntax_errors
        .iter()
        .map(|err| {
            rule_error(
                "GF000",
                format!("syntax error in `{}`: {}", doc.path, err.message),
                "Fix the malformed syntax reported by Tree-sitter before refactoring; never comment out broken code blocks to silence parse errors.",
                SourceLoc::new(&doc.path, err.span),
            )
        })
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}

pub fn check_file_metrics(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    let limits = config.limits;

    let enforce_line_cap = !matches!(
        doc.language,
        Language::Markdown
            | Language::Json
            | Language::Toml
            | Language::Svg
            | Language::BinaryAsset
    );
    if enforce_line_cap && doc.total_lines > limits.max_file_lines {
        diags.push(build_metric_error(MetricErrorSpec {
            rule_id: "GF001",
            message: format!(
                "file `{}` has {} lines (max {})",
                doc.path, doc.total_lines, limits.max_file_lines
            ),
            remediation: "Split cohesive types or functions into focused submodules; never compact multiple statements onto single lines or delete tests/docs to reduce line count.",
            measured: doc.total_lines,
            threshold: limits.max_file_lines,
            loc: SourceLoc::new(&doc.path, doc.root.span),
        }));
    }

    let import_count = u32::try_from(doc.imports.len()).unwrap_or(u32::MAX);
    if import_count > limits.max_imports_per_file {
        let span = doc.imports.first().map_or(doc.root.span, |imp| imp.span);
        diags.push(build_metric_error(MetricErrorSpec {
            rule_id: "GF007",
            message: format!(
                "file `{}` has {import_count} imports (max {})",
                doc.path, limits.max_imports_per_file
            ),
            remediation: "Reduce module fan-out by separating orchestration from domain logic into distinct modules instead of wildcard re-importing.",
            measured: import_count,
            threshold: limits.max_imports_per_file,
            loc: SourceLoc::new(&doc.path, span),
        }));
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

#[derive(Clone, Copy)]
struct CallableThresholdSpec {
    rule_id: &'static str,
    measured: u32,
    max: u32,
    label: &'static str,
    remediation: &'static str,
}

const fn callable_specs(callable: &CallableDecl, l: LimitsConfig) -> [CallableThresholdSpec; 5] {
    [
        CallableThresholdSpec {
            rule_id: "GF002",
            measured: callable.effective_lines,
            max: l.max_function_lines,
            label: "effective lines",
            remediation: "Extract cohesive pure helper functions; never collapse formatting onto single lines to game line counts.",
        },
        CallableThresholdSpec {
            rule_id: "GF003",
            measured: callable.param_count,
            max: l.max_parameters,
            label: "parameters",
            remediation: "Group cohesive parameters into a dedicated domain struct or context object; never pack unrelated positional values into raw tuples.",
        },
        CallableThresholdSpec {
            rule_id: "GF004",
            measured: callable.cognitive_complexity,
            max: l.max_cognitive_complexity,
            label: "cognitive complexity",
            remediation: "Flatten nested control flow with early guard clauses (`return` / `?`) and extract pure single-purpose helpers.",
        },
        CallableThresholdSpec {
            rule_id: "GF005",
            measured: callable.cyclomatic_complexity,
            max: l.max_cyclomatic_complexity,
            label: "cyclomatic complexity",
            remediation: "Decompose multi-branch conditionals into smaller domain functions or declarative lookup tables.",
        },
        CallableThresholdSpec {
            rule_id: "GF006",
            measured: callable.max_nesting_depth,
            max: l.max_nesting_depth,
            label: "nesting depth",
            remediation: "Invert outer conditions into early returns or extract inner loop/branch bodies into named helper functions.",
        },
    ]
}

fn check_single_callable(
    callable: &CallableDecl,
    path: &RepoPath,
    config: &Config,
) -> Vec<Diagnostic> {
    let name = &callable.name;
    callable_specs(callable, config.limits)
        .into_iter()
        .filter(|s| s.measured > s.max)
        .map(|s| {
            build_metric_error(MetricErrorSpec {
                rule_id: s.rule_id,
                message: format!(
                    "callable `{name}` has {} {} (max {})",
                    s.label, s.measured, s.max
                ),
                remediation: s.remediation,
                measured: s.measured,
                threshold: s.max,
                loc: SourceLoc::new(path, callable.span),
            })
        })
        .collect()
}

pub fn check_callables(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    let diags = doc
        .callables
        .iter()
        .flat_map(|c| check_single_callable(c, &doc.path, config))
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}
