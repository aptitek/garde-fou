//! Direct unit tests in `gf-rules` for cross-language `import_resolver.rs` via `GF072` boundaries.

use gf_core::{
    BoundaryRule, Config, ImportDecl, Language, ProjectContext, RepoPath, Span, UastDocument,
};
use gf_rules::evaluate_workspace_sorted;

type Res = Result<(), Box<dyn std::error::Error>>;

fn py_doc(
    path: &str,
    imports: Vec<ImportDecl>,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Python, 4, 4)?;
    doc.imports = imports;
    Ok(doc)
}

fn mk_imp(src: &str, syms: &[&str]) -> ImportDecl {
    ImportDecl {
        module_source: String::from(src),
        symbols: syms.iter().map(|&s| String::from(s)).collect(),
        is_type_only: false,
        span: Span::FIRST,
    }
}

fn check_denied(a_imp: ImportDecl, deny: &[&str]) -> Result<bool, Box<dyn std::error::Error>> {
    let docs = vec![
        py_doc("a.py", vec![a_imp])?,
        py_doc("pkg/__init__.py", Vec::new())?,
        py_doc("pkg/sub.py", Vec::new())?,
        py_doc("__init__.py", Vec::new())?,
        py_doc(".py", Vec::new())?,
    ];
    let mut cfg = Config::strict_defaults();
    cfg.boundaries = vec![BoundaryRule {
        from: String::from("a.py"),
        allow: Vec::new(),
        deny: deny.iter().map(|&s| String::from(s)).collect(),
        reason: String::from("test boundary"),
    }];
    let diags = evaluate_workspace_sorted(&docs, &ProjectContext::default(), &cfg);
    Ok(diags.iter().any(|d| d.rule_id == "GF072"))
}

#[test]
fn python_submodule_wildcard_and_root_relative_resolution_boundaries() -> Res {
    assert!(check_denied(mk_imp("pkg", &["sub"]), &["pkg/sub.py"])?);
    assert!(!check_denied(
        mk_imp("pkg", &["sub"]),
        &["pkg/__init__.py"]
    )?);

    assert!(check_denied(mk_imp("pkg", &["*"]), &["pkg/__init__.py"])?);
    assert!(!check_denied(mk_imp("pkg", &["*"]), &["pkg/sub.py"])?);

    assert!(check_denied(mk_imp("pkg", &[""]), &["pkg/__init__.py"])?);
    assert!(!check_denied(mk_imp(".", &[""]), &[".py"])?);

    assert!(!check_denied(
        mk_imp(".", &["missing"]),
        &["__init__.py", "pkg/__init__.py", "pkg/sub.py"]
    )?);
    assert!(check_denied(mk_imp(".", &["pkg.sub"]), &["pkg/sub.py"])?);
    Ok(())
}
