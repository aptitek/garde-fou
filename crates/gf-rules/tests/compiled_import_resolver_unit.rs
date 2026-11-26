//! Unit tests for Go/Kotlin/C# workspace import resolution (`GF071`) and Go implicit
//! structural interface satisfaction (`GF075`) through `gf_rules::evaluate_workspace`.

use std::collections::BTreeSet;

use gf_core::{
    CompiledModuleSummary, ConcreteTypeMethods, Config, ContractDecl, Diagnostic, ImportDecl,
    Language, ProjectContext, RepoPath, Span, StructuralInterfaceDecl, UastDocument,
};
use gf_rules::evaluate_workspace;

fn mk_doc(
    path: &str,
    lang: Language,
    meta: Option<CompiledModuleSummary>,
    imports: &[&str],
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, lang, 1, 1)?;
    doc.compiled_meta = meta;
    for &src in imports {
        doc.imports.push(ImportDecl {
            module_source: String::from(src),
            symbols: Vec::new(),
            span: Span::FIRST,
            is_type_only: false,
        });
    }
    Ok(doc)
}

fn mk_pkg_doc(
    path: &str,
    lang: Language,
    pkg: &str,
    imports: &[&str],
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let meta = CompiledModuleSummary {
        package_or_namespace: Some(String::from(pkg)),
        ..CompiledModuleSummary::default()
    };
    mk_doc(path, lang, Some(meta), imports)
}

fn eval_diags(docs: &[UastDocument]) -> Vec<Diagnostic> {
    evaluate_workspace(docs, &ProjectContext::default(), &Config::strict_defaults())
        .into_result()
        .err()
        .unwrap_or_default()
}

fn mk_iface(name: &str, methods: &[&str]) -> StructuralInterfaceDecl {
    StructuralInterfaceDecl {
        name: String::from(name),
        span: Span::FIRST,
        methods: methods.iter().map(|&m| String::from(m)).collect(),
    }
}

fn mk_concrete(name: &str, methods: &[&str]) -> ConcreteTypeMethods {
    ConcreteTypeMethods {
        type_name: String::from(name),
        methods: methods.iter().map(|&m| String::from(m)).collect(),
    }
}

#[test]
fn resolves_go_root_and_subpkg_cycles_while_excluding_go_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let go_mod = CompiledModuleSummary {
        go_module_path: Some(String::from("acme.io/app")),
        ..CompiledModuleSummary::default()
    };
    let docs = vec![
        mk_doc("go.mod", Language::Unknown, Some(go_mod), &[])?,
        mk_doc(
            "root.go",
            Language::Go,
            None,
            &["\"acme.io/app/internal/svc\""],
        )?,
        mk_doc(
            "internal/svc/svc.go",
            Language::Go,
            None,
            &["\"acme.io/app\""],
        )?,
        mk_doc(
            "internal/onlytest/a_test.go",
            Language::Go,
            None,
            &["\"acme.io/app/solo\""],
        )?,
        mk_doc(
            "solo/main.go",
            Language::Go,
            None,
            &["\"acme.io/app/internal/onlytest\""],
        )?,
    ];
    let gf071: Vec<String> = eval_diags(&docs)
        .into_iter()
        .filter(|d| d.rule_id == "GF071")
        .map(|d| String::from(d.path.as_str()))
        .collect();
    assert!(gf071.iter().any(|p| p == "root.go"));
    assert!(gf071.iter().any(|p| p == "internal/svc/svc.go"));
    assert!(!gf071.iter().any(|p| p == "internal/onlytest/a_test.go"));
    assert!(!gf071.iter().any(|p| p == "solo/main.go"));
    Ok(())
}

#[test]
fn resolves_kotlin_and_csharp_package_namespace_cycles() -> Result<(), Box<dyn std::error::Error>> {
    let docs = vec![
        mk_pkg_doc(
            "src/Domain.kt",
            Language::Kotlin,
            "com.acme.domain",
            &["com.acme.infra.Repo"],
        )?,
        mk_pkg_doc(
            "src/Infra.kt",
            Language::Kotlin,
            "com.acme.infra",
            &["com.acme.domain.*"],
        )?,
        mk_pkg_doc(
            "src/Billing.cs",
            Language::CSharp,
            "Acme.Billing",
            &["Acme.Ledger;"],
        )?,
        mk_pkg_doc(
            "src/Ledger.cs",
            Language::CSharp,
            "Acme.Ledger",
            &["Acme.Billing"],
        )?,
    ];
    let gf071: Vec<String> = eval_diags(&docs)
        .into_iter()
        .filter(|d| d.rule_id == "GF071")
        .map(|d| String::from(d.path.as_str()))
        .collect();
    assert!(gf071.iter().any(|p| p == "src/Domain.kt"));
    assert!(gf071.iter().any(|p| p == "src/Billing.cs"));
    Ok(())
}

#[test]
fn augments_go_structural_interface_implementors_for_gf075()
-> Result<(), Box<dyn std::error::Error>> {
    let iface_meta = CompiledModuleSummary {
        go_interfaces: vec![
            mk_iface("Store", &["Get", "Put"]),
            mk_iface("Single", &["Only"]),
            mk_iface("Empty", &[]),
        ],
        ..CompiledModuleSummary::default()
    };
    let mut iface_doc = mk_doc(
        "internal/store/iface.go",
        Language::Go,
        Some(iface_meta),
        &[],
    )?;
    for (name, method_count) in [("Store", 2), ("Single", 1)] {
        iface_doc.contracts.push(ContractDecl {
            name: String::from(name),
            span: Span::FIRST,
            method_count,
            is_public: true,
        });
    }
    let impl_meta = CompiledModuleSummary {
        go_concrete_types: vec![
            mk_concrete("MemStore", &["Get", "Put"]),
            mk_concrete("DiskStore", &["Get", "Put"]),
            mk_concrete("OneImpl", &["Only", "Get"]),
        ],
        ..CompiledModuleSummary::default()
    };
    let impl_doc = mk_doc(
        "internal/store/impls.go",
        Language::Go,
        Some(impl_meta),
        &[],
    )?;
    let gf075: BTreeSet<String> = eval_diags(&[iface_doc, impl_doc])
        .into_iter()
        .filter(|d| d.rule_id == "GF075")
        .map(|d| d.message)
        .collect();
    assert!(gf075.iter().any(|m| m.contains("Single")));
    assert!(!gf075.iter().any(|m| m.contains("Store")));
    Ok(())
}
