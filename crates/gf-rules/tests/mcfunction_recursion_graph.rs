//! Workspace `.mcfunction` mutual recursion / cycle detection (`GF071`).

use gf_core::{
    Config, ImportDecl, Language, ProjectContext, RepoPath, Span, UastDocument, Validation,
};
use gf_rules::evaluate_workspace;

fn mcfunc_doc(path: &str, calls: &[&str]) -> Result<UastDocument, String> {
    let rp = RepoPath::new(path).map_err(|e| e.to_string())?;
    let mut doc = UastDocument::new(rp, Language::McFunction, 4, 3).map_err(|e| e.to_string())?;
    for target in calls {
        doc.imports.push(ImportDecl {
            module_source: String::from(*target),
            symbols: Vec::new(),
            is_type_only: false,
            span: Span::FIRST,
        });
    }
    Ok(doc)
}

#[test]
fn mcfunction_mutual_recursion_cycle_and_acyclic_pass() -> Result<(), String> {
    let cyclic = [
        mcfunc_doc("data/demo/function/a.mcfunction", &["demo:b"])?,
        mcfunc_doc("data/demo/function/b.mcfunction", &["demo:a"])?,
    ];
    let res = evaluate_workspace(&cyclic, &ProjectContext::default(), &Config::default());
    assert!(
        matches!(res, Validation::Invalid(ref errs) if errs.iter().any(|d| d.rule_id.as_str() == "GF071")),
        "expected GF071 cycle in {res:?}"
    );
    let acyclic = [
        mcfunc_doc("data/demo/function/tick.mcfunction", &["demo:helper"])?,
        mcfunc_doc("data/demo/function/helper.mcfunction", &[])?,
    ];
    let ok = evaluate_workspace(&acyclic, &ProjectContext::default(), &Config::default());
    assert!(
        !matches!(ok, Validation::Invalid(ref errs) if errs.iter().any(|d| d.rule_id.as_str() == "GF071")),
        "unexpected GF071 on acyclic datapack: {ok:?}"
    );
    Ok(())
}
