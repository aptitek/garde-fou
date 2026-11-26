//! Workspace PSR-4 cycle detection (`GF071`) for PHP / Symfony projects.

use gf_core::{
    Config, ImportDecl, Language, ProjectContext, RepoPath, Span, UastDocument, Validation,
};
use gf_rules::evaluate_workspace;

#[test]
fn php_psr4_cycle_is_detected_by_gf071() -> Result<(), String> {
    let a = RepoPath::new("src/Service/Alpha.php").map_err(|e| e.to_string())?;
    let b = RepoPath::new("src/Service/Beta.php").map_err(|e| e.to_string())?;
    let mut da = UastDocument::new(a, Language::Php, 10, 8).map_err(|e| e.to_string())?;
    da.imports.push(ImportDecl {
        module_source: String::from("App\\Service\\Beta"),
        symbols: Vec::new(),
        is_type_only: false,
        span: Span::FIRST,
    });
    let mut db = UastDocument::new(b, Language::Php, 10, 8).map_err(|e| e.to_string())?;
    db.imports.push(ImportDecl {
        module_source: String::from("App\\Service\\Alpha"),
        symbols: Vec::new(),
        is_type_only: false,
        span: Span::FIRST,
    });
    let diags = evaluate_workspace(&[da, db], &ProjectContext::default(), &Config::default());
    let found = matches!(
        diags,
        Validation::Invalid(ref errs) if errs.iter().any(|d| d.rule_id.as_str() == "GF071")
    );
    assert!(found, "expected GF071 cycle in {diags:?}");
    Ok(())
}
