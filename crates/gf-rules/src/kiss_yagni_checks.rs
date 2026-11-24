//! Pure `#![no_std]` anti-bypass (`GF070`), KISS (`GF073`), local YAGNI (`GF074`),
//! and workspace single-implementation contract (`GF075`) evaluators.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, KissYagniFinding, KissYagniKind, ProjectContext, RepoPath,
    Severity, Span, UastDocument, Validation, classify_path,
};

use crate::checks::rule_error;
use crate::workspace_checks::is_entrypoint_file;

fn measured_diag(
    rule_id: &str,
    message: String,
    remediation: &str,
    ctx: (&RepoPath, Span, u32, u32),
) -> Diagnostic {
    let (path, span, measured, threshold) = ctx;
    Diagnostic {
        rule_id: String::from(rule_id),
        severity: Severity::Error,
        message,
        remediation: String::from(remediation),
        measured,
        threshold,
        path: path.clone(),
        span,
    }
}

fn evaluate_kiss_item(
    doc: &UastDocument,
    item: &KissYagniFinding,
    config: &Config,
) -> Option<Diagnostic> {
    let cc = &config.clean_code;
    match item.kind {
        KissYagniKind::NestedTernary if item.measured > cc.max_nested_ternaries => {
            Some(measured_diag(
                "GF073",
                format!(
                    "KISS violation: {} (max {})",
                    item.detail, cc.max_nested_ternaries
                ),
                "Replace nested ternary expressions with early-return helper functions or explicit `if`/`match` branches.",
                (&doc.path, item.span, item.measured, cc.max_nested_ternaries),
            ))
        }
        KissYagniKind::PropertyChain if item.measured > cc.max_property_chain => {
            Some(measured_diag(
                "GF073",
                format!(
                    "KISS violation: {} (max {})",
                    item.detail, cc.max_property_chain
                ),
                "Respect the Law of Demeter: expose a domain method on the owning type instead of chaining raw internal field accesses.",
                (&doc.path, item.span, item.measured, cc.max_property_chain),
            ))
        }
        _ => None,
    }
}

fn evaluate_yagni_item(
    doc: &UastDocument,
    domain: DomainKind,
    item: &KissYagniFinding,
    config: &Config,
) -> Option<Diagnostic> {
    let cc = &config.clean_code;
    match item.kind {
        KissYagniKind::WildcardReExport if cc.yagni.forbid_wildcard_exports => Some(rule_error(
            "GF074",
            format!("YAGNI barrel/wildcard violation: {}", item.detail),
            "Replace wildcard `export *` / `import *` / `pub use *` with explicit named symbol imports/exports.",
            (&doc.path, item.span),
        )),
        KissYagniKind::ExcessiveBoolParams if item.measured > cc.max_bool_parameters => {
            Some(measured_diag(
                "GF074",
                format!(
                    "YAGNI boolean-flag overload: {} (max {})",
                    item.detail, cc.max_bool_parameters
                ),
                "Replace multiple `bool` flag parameters with a dedicated `enum` or small typed options struct.",
                (&doc.path, item.span, item.measured, cc.max_bool_parameters),
            ))
        }
        KissYagniKind::TodoMarker | KissYagniKind::CommentedOutCode
            if cc.forbid_todos && domain == DomainKind::Code =>
        {
            Some(rule_error(
                "GF074",
                format!("unresolved code debt in `Code`: {}", item.detail),
                "Resolve deferred work immediately or track it in your issue tracker instead of leaving `TODO`/`FIXME` comments in `Code`.",
                (&doc.path, item.span),
            ))
        }
        _ => None,
    }
}

/// Evaluates single-file anti-bypass (`GF070`), KISS (`GF073`), and local YAGNI (`GF074`).
pub fn check_local_kiss_yagni_and_bypass(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let domain = classify_path(&doc.path, &config.domains);
    let mut diags = Vec::new();

    for bypass in &doc.bypass_findings {
        diags.push(rule_error(
            "GF070",
            format!(
                "linter/typechecker bypass `{}` is strictly forbidden in `{}`",
                bypass.directive, doc.path
            ),
            concat!(
                "Fix the underlying root cause instead of silencing static analysis with inline ignore comments or `#[",
                "allow(...)]`."
            ),
            (&doc.path, bypass.span),
        ));
    }

    for item in &doc.kiss_yagni_findings {
        if let Some(diag) = evaluate_kiss_item(doc, item, config)
            .or_else(|| evaluate_yagni_item(doc, domain, item, config))
        {
            diags.push(diag);
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn build_contract_impl_counts(docs: &[UastDocument]) -> BTreeMap<&str, BTreeSet<&str>> {
    let mut impl_counts: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for doc in docs {
        for binding in &doc.contract_impls {
            impl_counts
                .entry(binding.contract.as_str())
                .or_default()
                .insert(binding.implementor.as_str());
        }
    }
    impl_counts
}

/// Evaluates workspace-wide single-implementation behavioral abstractions (`GF075`).
pub fn check_single_impl_contracts(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    if !config.clean_code.yagni.forbid_single_impl_traits {
        return Validation::Valid(());
    }
    let impl_counts = build_contract_impl_counts(docs);
    let mut diags = Vec::new();
    for doc in docs {
        if classify_path(&doc.path, &config.domains) != DomainKind::Code
            || is_entrypoint_file(&doc.path, ctx, config)
        {
            continue;
        }
        for contract in &doc.contracts {
            let count = impl_counts
                .get(contract.name.as_str())
                .map_or(0, BTreeSet::len);
            if count <= 1 {
                diags.push(measured_diag(
                    "GF075",
                    format!(
                        "YAGNI single-implementation abstraction: `{}` in `{}` has {count} concrete implementation(s) (min 2)",
                        contract.name, doc.path
                    ),
                    "Use a concrete struct/class/function directly until at least two genuine implementations exist, or expose public library extension traits from a declared entrypoint.",
                    (&doc.path, contract.span, u32::try_from(count).unwrap_or(0), 2),
                ));
            }
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
