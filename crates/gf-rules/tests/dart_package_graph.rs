//! Workspace `package:` and relative import cycle detection (`GF071`) for Dart & Flutter projects.

use gf_core::{
    Config, ImportDecl, Language, ProjectContext, RepoPath, Span, UastDocument, Validation,
};
use gf_rules::evaluate_workspace;

fn dart_doc(path: &str, imports: &[&str]) -> Result<UastDocument, String> {
    let rp = RepoPath::new(path).map_err(|e| e.to_string())?;
    let mut doc = UastDocument::new(rp, Language::Dart, 6, 5).map_err(|e| e.to_string())?;
    for spec in imports {
        doc.imports.push(ImportDecl {
            module_source: String::from(*spec),
            symbols: Vec::new(),
            is_type_only: false,
            span: Span::FIRST,
        });
    }
    Ok(doc)
}

#[test]
fn dart_package_and_relative_import_graph_checks_cycles() -> Result<(), String> {
    let docs = [
        dart_doc("lib/a.dart", &["package:myapp/b.dart"])?,
        dart_doc("lib/b.dart", &["a.dart"])?,
        dart_doc("lib/leaf.dart", &[])?,
    ];
    let diags = evaluate_workspace(&docs, &ProjectContext::default(), &Config::default());
    let has_cycle = matches!(
        diags,
        Validation::Invalid(ref errs) if errs.iter().any(|d| d.rule_id.as_str() == "GF071")
    );
    assert!(has_cycle, "expected GF071 cycle in {diags:?}");
    Ok(())
}
