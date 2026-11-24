//! Pure `#![no_std]` anti-bypass (`GF070`), KISS (`GF073`), local YAGNI (`GF074`),
//! and workspace single-implementation contract (`GF075`) evaluators.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, KissYagniFinding, KissYagniKind, ProjectContext, Severity,
    UastDocument, Validation, classify_path,
};

use crate::checks::{SourceLoc, rule_error};
use crate::workspace_checks::is_entrypoint_file;

struct MeasuredSpec<'a> {
    rule_id: &'static str,
    message: String,
    remediation: &'static str,
    loc: SourceLoc<'a>,
    measured: u32,
    threshold: u32,
}

fn measured_diag(spec: MeasuredSpec<'_>) -> Diagnostic {
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

fn evaluate_kiss_item(
    doc: &UastDocument,
    item: &KissYagniFinding,
    config: &Config,
) -> Option<Diagnostic> {
    let cc = &config.clean_code;
    let loc = SourceLoc::new(&doc.path, item.span);
    match item.kind {
        KissYagniKind::NestedTernary if item.measured > cc.max_nested_ternaries => {
            Some(measured_diag(MeasuredSpec {
                rule_id: "GF073",
                message: format!(
                    "KISS violation: {} (max {})",
                    item.detail, cc.max_nested_ternaries
                ),
                remediation: "Replace nested ternary expressions with early-return helper functions or explicit `if`/`match` branches.",
                loc,
                measured: item.measured,
                threshold: cc.max_nested_ternaries,
            }))
        }
        KissYagniKind::PropertyChain if item.measured > cc.max_property_chain => {
            Some(measured_diag(MeasuredSpec {
                rule_id: "GF073",
                message: format!(
                    "KISS violation: {} (max {})",
                    item.detail, cc.max_property_chain
                ),
                remediation: "Respect the Law of Demeter: expose a domain method on the owning type instead of chaining raw internal field accesses.",
                loc,
                measured: item.measured,
                threshold: cc.max_property_chain,
            }))
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
    let loc = SourceLoc::new(&doc.path, item.span);
    match item.kind {
        KissYagniKind::WildcardReExport if cc.has_wildcard_exports() => Some(rule_error(
            "GF074",
            format!("YAGNI barrel/wildcard violation: {}", item.detail),
            "Replace wildcard `export *` / `import *` / `pub use *` with explicit named symbol imports/exports.",
            loc,
        )),
        KissYagniKind::ExcessiveBoolParams if item.measured > cc.max_bool_parameters => {
            Some(measured_diag(MeasuredSpec {
                rule_id: "GF074",
                message: format!(
                    "YAGNI boolean-flag overload: {} (max {})",
                    item.detail, cc.max_bool_parameters
                ),
                remediation: "Replace multiple `bool` flag parameters with a dedicated `enum` or small typed options struct.",
                loc,
                measured: item.measured,
                threshold: cc.max_bool_parameters,
            }))
        }
        KissYagniKind::TodoMarker | KissYagniKind::CommentedOutCode
            if cc.forbid_todos && domain == DomainKind::Code =>
        {
            Some(rule_error(
                "GF074",
                format!("unresolved code debt in `Code`: {}", item.detail),
                "Resolve deferred work immediately or delete commented-out code instead of leaving `TODO`/`FIXME` or dead code blocks in `Code`.",
                loc,
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
            "Fix the underlying root cause instead of silencing static analysis with inline ignore comments or `#[allow(...)]`.",
            SourceLoc::new(&doc.path, bypass.span),
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
    if !config.clean_code.has_single_impl_traits() {
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
                diags.push(measured_diag(MeasuredSpec {
                    rule_id: "GF075",
                    message: format!(
                        "YAGNI single-implementation abstraction: `{}` in `{}` has {count} concrete implementation(s) (min 2)",
                        contract.name, doc.path
                    ),
                    remediation: "Use a concrete struct/class/function directly until at least two genuine implementations exist, or expose public library extension traits from a declared entrypoint.",
                    loc: SourceLoc::new(&doc.path, contract.span),
                    measured: u32::try_from(count).unwrap_or(0),
                    threshold: 2,
                }));
            }
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
