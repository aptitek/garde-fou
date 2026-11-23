//! Pure `#![no_std]` rule evaluation engine for **Garde-fou**.
//!
//! Depends exclusively on [`gf_core`] and transforms a [`UastDocument`] plus [`Config`]
//! into deterministic [`Diagnostic`] values without performing any I/O.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{Config, Diagnostic, RepoPath, Severity, Span, UastDocument, UastKind, UastNode};

#[derive(Clone, Copy)]
struct EvalContext<'a> {
    path: &'a RepoPath,
    config: &'a Config,
}

fn error_diagnostic(rule_id: &str, message: String, path: &RepoPath, span: Span) -> Diagnostic {
    Diagnostic {
        rule_id: String::from(rule_id),
        severity: Severity::Error,
        message,
        path: path.clone(),
        span,
    }
}

fn evaluate_function(
    name: &str,
    metrics: (u32, u32),
    span: Span,
    ctx: EvalContext<'_>,
) -> Vec<Diagnostic> {
    let (param_count, cognitive_complexity) = metrics;
    let mut diagnostics = Vec::new();
    let lines = span.line_count();
    let limits = ctx.config.limits;

    if lines > limits.max_function_lines {
        diagnostics.push(error_diagnostic(
            "GF002",
            format!(
                "function `{name}` has {lines} lines (max {})",
                limits.max_function_lines
            ),
            ctx.path,
            span,
        ));
    }
    if param_count > limits.max_function_params {
        diagnostics.push(error_diagnostic(
            "GF003",
            format!(
                "function `{name}` has {param_count} parameters (max {})",
                limits.max_function_params
            ),
            ctx.path,
            span,
        ));
    }
    if cognitive_complexity > limits.max_cognitive_complexity {
        diagnostics.push(error_diagnostic(
            "GF004",
            format!(
                "function `{name}` has cognitive complexity {cognitive_complexity} (max {})",
                limits.max_cognitive_complexity
            ),
            ctx.path,
            span,
        ));
    }
    diagnostics
}

fn evaluate_node(node: &UastNode, ctx: EvalContext<'_>, out: &mut Vec<Diagnostic>) {
    match &node.kind {
        UastKind::Function {
            name,
            param_count,
            cognitive_complexity,
        } => {
            out.extend(evaluate_function(
                name,
                (*param_count, *cognitive_complexity),
                node.span,
                ctx,
            ));
        }
        UastKind::ForbiddenCall { symbol } if ctx.config.forbid_panicking_constructs => {
            out.push(error_diagnostic(
                "GF010",
                format!("forbidden construct `{symbol}` is denied"),
                ctx.path,
                node.span,
            ));
        }
        UastKind::File | UastKind::ForbiddenCall { .. } => {}
    }

    for child in &node.children {
        evaluate_node(child, ctx, out);
    }
}

/// Evaluates all pure static rules against a normalized [`UastDocument`].
#[must_use]
pub fn evaluate_document(doc: &UastDocument, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if doc.total_lines > config.limits.max_file_lines {
        diagnostics.push(error_diagnostic(
            "GF001",
            format!(
                "file `{}` has {} lines (max {})",
                doc.path, doc.total_lines, config.limits.max_file_lines
            ),
            &doc.path,
            doc.root.span,
        ));
    }

    let ctx = EvalContext {
        path: &doc.path,
        config,
    };
    evaluate_node(&doc.root, ctx, &mut diagnostics);
    diagnostics
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::string::ToString;
    use alloc::vec;

    use gf_core::{Config, Language, RepoPath, Span, UastDocument, UastKind, UastNode};

    use super::evaluate_document;

    #[test]
    fn clean_document_emits_zero_diagnostics() -> Result<(), Box<dyn core::error::Error>> {
        let doc = UastDocument {
            path: RepoPath::new("src/clean.rs")?,
            language: Language::Rust,
            total_lines: 40,
            root: UastNode {
                kind: UastKind::File,
                span: Span::new(1, 40)?,
                children: vec![UastNode::leaf(
                    UastKind::Function {
                        name: "ok".to_string(),
                        param_count: 2,
                        cognitive_complexity: 4,
                    },
                    Span::new(5, 20)?,
                )],
            },
        };
        assert_eq!(evaluate_document(&doc, &Config::strict_defaults()), vec![]);
        Ok(())
    }

    #[test]
    fn flags_srp_and_forbidden_call_violations() -> Result<(), Box<dyn core::error::Error>> {
        let bad_fn = UastNode {
            kind: UastKind::Function {
                name: "bloated".to_string(),
                param_count: 6,
                cognitive_complexity: 14,
            },
            span: Span::new(1, 60)?,
            children: vec![UastNode::leaf(
                UastKind::ForbiddenCall {
                    symbol: "unwrap".to_string(),
                },
                Span::new(12, 12)?,
            )],
        };
        let doc = UastDocument {
            path: RepoPath::new("src/bloated.rs")?,
            language: Language::Rust,
            total_lines: 260,
            root: UastNode {
                kind: UastKind::File,
                span: Span::new(1, 260)?,
                children: vec![bad_fn],
            },
        };
        let ids: vec::Vec<alloc::string::String> =
            evaluate_document(&doc, &Config::strict_defaults())
                .into_iter()
                .map(|d| d.rule_id)
                .collect();
        assert_eq!(ids, vec!["GF001", "GF002", "GF003", "GF004", "GF010"]);
        Ok(())
    }
}
