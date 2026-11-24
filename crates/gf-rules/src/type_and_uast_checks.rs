//! Type SRP (`GF008`–`GF009`) and granular forbidden-construct (`GF010`–`GF011`) evaluators.

use alloc::format;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, ForbiddenRuleKind, RepoPath, UastDocument, UastKind, UastNode, Validation,
};

use crate::checks::{MetricErrorSpec, SourceLoc, build_metric_error, rule_error};

pub fn check_types(doc: &UastDocument, config: &Config) -> Validation<(), Diagnostic> {
    let max_methods = config.limits.max_methods_per_type;
    let max_bools = config.limits.max_struct_bools;
    let mut diags = Vec::new();

    for ty in &doc.types {
        if ty.method_count > max_methods {
            diags.push(build_metric_error(MetricErrorSpec {
                rule_id: "GF008",
                message: format!(
                    "type `{}` defines {} methods (max {max_methods})",
                    ty.name, ty.method_count
                ),
                remediation: "Split this God type/impl into smaller single-responsibility types or focused trait implementations.",
                measured: ty.method_count,
                threshold: max_methods,
                loc: SourceLoc::new(&doc.path, ty.span),
            }));
        }
        if ty.bool_field_count > max_bools {
            diags.push(build_metric_error(MetricErrorSpec {
                rule_id: "GF009",
                message: format!(
                    "type `{}` declares {} boolean fields (max {max_bools})",
                    ty.name, ty.bool_field_count
                ),
                remediation: "Replace multiple boolean flags on a struct with a domain `enum` (sum type) or a `BTreeSet<FlagEnum>` instead of nesting boolean helper structs.",
                measured: ty.bool_field_count,
                threshold: max_bools,
                loc: SourceLoc::new(&doc.path, ty.span),
            }));
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn walk_uast_node(node: &UastNode, path: &RepoPath, config: &Config, out: &mut Vec<Diagnostic>) {
    if let UastKind::ForbiddenCall { rule, symbol } = &node.kind
        && config.rules.contains(*rule)
    {
        let loc = SourceLoc::new(path, node.span);
        if *rule == ForbiddenRuleKind::SilentErrorSwallow {
            out.push(rule_error(
                "GF011",
                format!("silent error swallowing `{symbol}` is denied"),
                "Propagate or handle the error explicitly instead of discarding `Result`/exceptions with `let _ = ...`, silent closures, or fallback defaults.",
                loc,
            ));
        } else {
            out.push(rule_error(
                "GF010",
                format!("forbidden construct `{symbol}` (`{rule}`) is denied"),
                "Propagate recoverable errors with `Result<T, E>` and `?` instead of panicking or using `unsafe`.",
                loc,
            ));
        }
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
