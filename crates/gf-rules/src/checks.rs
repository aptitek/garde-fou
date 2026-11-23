//! Individual pure SRP, complexity, and syntax rule evaluators.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    CallableDecl, Config, Diagnostic, Language, RepoPath, Severity, Span, TypeDecl, UastDocument,
    UastKind, UastNode, Validation,
};

struct Spec<'a> {
    rule_id: &'static str,
    message: String,
    remediation: &'static str,
    bounds: (u32, u32),
    path: &'a RepoPath,
    span: Span,
}

fn build_error(spec: Spec<'_>) -> Diagnostic {
    let (measured, threshold) = spec.bounds;
    Diagnostic {
        rule_id: String::from(spec.rule_id),
        severity: Severity::Error,
        message: spec.message,
        remediation: String::from(spec.remediation),
        measured,
        threshold,
        path: spec.path.clone(),
        span: spec.span,
    }
}

pub fn check_syntax_errors(doc: &UastDocument) -> Validation<(), Diagnostic> {
    let diags = doc
        .syntax_errors
        .iter()
        .map(|err| {
            build_error(Spec {
                rule_id: "GF000",
                message: format!("syntax error in `{}`: {}", doc.path, err.message),
                remediation: "Fix the malformed syntax reported by Tree-sitter before refactoring; never comment out broken code blocks to silence parse errors.",
                bounds: (1, 0),
                path: &doc.path,
                span: err.span,
            })
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
        diags.push(build_error(Spec {
            rule_id: "GF001",
            message: format!(
                "file `{}` has {} lines (max {})",
                doc.path, doc.total_lines, limits.max_file_lines
            ),
            remediation: "Split cohesive types or functions into focused submodules; never compact multiple statements onto single lines or delete tests/docs to reduce line count.",
            bounds: (doc.total_lines, limits.max_file_lines),
            path: &doc.path,
            span: doc.root.span,
        }));
    }

    let import_count = u32::try_from(doc.imports.len()).unwrap_or(u32::MAX);
    if import_count > limits.max_imports_per_file {
        let span = doc.imports.first().map_or(doc.root.span, |imp| imp.span);
        diags.push(build_error(Spec {
            rule_id: "GF007",
            message: format!(
                "file `{}` has {import_count} imports (max {})",
                doc.path, limits.max_imports_per_file
            ),
            remediation: "Reduce module fan-out by separating orchestration from domain logic into distinct modules instead of wildcard re-importing.",
            bounds: (import_count, limits.max_imports_per_file),
            path: &doc.path,
            span,
        }));
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

type CallableCheckSpec = (&'static str, u32, u32, &'static str, &'static str);

const fn callable_specs(
    callable: &CallableDecl,
    l: gf_core::LimitsConfig,
) -> [CallableCheckSpec; 5] {
    [
        (
            "GF002",
            callable.effective_lines,
            l.max_function_lines,
            "effective lines",
            "Extract cohesive pure helper functions; never collapse formatting onto single lines to game line counts.",
        ),
        (
            "GF003",
            callable.param_count,
            l.max_parameters,
            "parameters",
            "Group cohesive parameters into a dedicated domain struct or context object; never pack unrelated positional values into raw tuples.",
        ),
        (
            "GF004",
            callable.cognitive_complexity,
            l.max_cognitive_complexity,
            "cognitive complexity",
            "Flatten nested control flow with early guard clauses (`return` / `?`) and extract pure single-purpose helpers.",
        ),
        (
            "GF005",
            callable.cyclomatic_complexity,
            l.max_cyclomatic_complexity,
            "cyclomatic complexity",
            "Decompose multi-branch conditionals into smaller domain functions or declarative lookup tables.",
        ),
        (
            "GF006",
            callable.max_nesting_depth,
            l.max_nesting_depth,
            "nesting depth",
            "Invert outer conditions into early returns or extract inner loop/branch bodies into named helper functions.",
        ),
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
        .filter(|(_, measured, max, _, _)| *measured > *max)
        .map(|(rule_id, measured, max, label, remediation)| {
            build_error(Spec {
                rule_id,
                message: format!("callable `{name}` has {label} {measured} (max {max})"),
                remediation,
                bounds: (measured, max),
                path,
                span: callable.span,
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

pub fn check_types(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    let max = config.limits.max_methods_per_type;
    let diags = doc
        .types
        .iter()
        .filter(|ty: &&TypeDecl| ty.method_count > max)
        .map(|ty| {
            build_error(Spec {
                rule_id: "GF008",
                message: format!(
                    "type `{}` defines {} methods (max {max})",
                    ty.name, ty.method_count
                ),
                remediation: "Split this God type/impl into smaller single-responsibility types or focused trait implementations.",
                bounds: (ty.method_count, max),
                path: &doc.path,
                span: ty.span,
            })
        })
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn walk_uast_node(node: &UastNode, path: &RepoPath, config: &Config, out: &mut Vec<Diagnostic>) {
    if let UastKind::ForbiddenCall { symbol } = &node.kind
        && config.forbid_panicking_constructs
    {
        out.push(build_error(Spec {
            rule_id: "GF010",
            message: format!("forbidden construct `{symbol}` is denied"),
            remediation: "Propagate recoverable errors with `Result<T, E>` and `?` instead of panicking or using `unsafe`.",
            bounds: (1, 0),
            path,
            span: node.span,
        }));
    }
    for child in &node.children {
        walk_uast_node(child, path, config, out);
    }
}

pub fn check_root_uast(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    walk_uast_node(&doc.root, &doc.path, config, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
