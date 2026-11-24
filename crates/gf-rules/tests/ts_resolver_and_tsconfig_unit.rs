//! Direct unit tests in `gf-rules` for `ts_import_resolver.rs` and `check_workspace_tsconfigs`
//! (`tool_checks.rs`), ensuring 100% mutation coverage inside `gf-rules`.

use std::collections::BTreeMap;

use gf_core::{
    BoundaryRule, Config, ImportDecl, Language, ProjectContext, RepoPath, Span,
    TsconfigStrictState, TsconfigSummary, UastDocument,
};
use gf_rules::evaluate_workspace_sorted;

type Res = Result<(), Box<dyn std::error::Error>>;

fn ts_doc(
    path: &str,
    imports: Vec<ImportDecl>,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::TypeScript, 5, 5)?;
    doc.imports = imports;
    Ok(doc)
}

fn strict_tsconfig_doc(
    path: &str,
    extends: Option<&str>,
    strict: TsconfigStrictState,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Json, 5, 5)?;
    doc.tsconfig = Some(TsconfigSummary {
        extends: extends.map(String::from),
        strict,
        base_url: None,
        paths: BTreeMap::new(),
        strict_span: Span::FIRST,
    });
    Ok(doc)
}

fn alias_tsconfig_doc(
    path: &str,
    base_url: Option<&str>,
    paths: &[(&str, &[&str])],
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Json, 5, 5)?;
    let paths_map: BTreeMap<String, Vec<String>> = paths
        .iter()
        .map(|(k, v)| {
            (
                String::from(*k),
                v.iter().map(|s| String::from(*s)).collect(),
            )
        })
        .collect();
    doc.tsconfig = Some(TsconfigSummary {
        extends: None,
        strict: TsconfigStrictState::ExplicitTrue,
        base_url: base_url.map(String::from),
        paths: paths_map,
        strict_span: Span::FIRST,
    });
    Ok(doc)
}

fn imp(spec: &str) -> ImportDecl {
    ImportDecl {
        module_source: String::from(spec),
        symbols: Vec::new(),
        is_type_only: false,
        span: Span::FIRST,
    }
}

#[test]
fn valid_tsconfig_extends_chains_and_presets_emit_zero_gf081() -> Res {
    let ok_docs = vec![
        strict_tsconfig_doc("sub/tsconfig.json", None, TsconfigStrictState::ExplicitTrue)?,
        strict_tsconfig_doc(
            "tsconfig.mid.json",
            Some("./sub"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc(
            "tsconfig.leaf.json",
            Some("./tsconfig.mid"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc(
            "tsconfig.preset.json",
            Some("@tsconfig/strictest"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc("tsconfig.no_ext.json", None, TsconfigStrictState::Omitted)?,
    ];
    let gf081: Vec<_> = evaluate_workspace_sorted(
        &ok_docs,
        &ProjectContext::default(),
        &Config::strict_defaults(),
    )
    .into_iter()
    .filter(|d| d.rule_id == "GF081")
    .collect();
    assert_eq!(gf081, Vec::new());
    Ok(())
}

#[test]
fn broken_or_cyclic_tsconfig_extends_chains_emit_exact_gf081_paths() -> Res {
    let bad_docs = vec![
        strict_tsconfig_doc(
            "tsconfig.false_base.json",
            None,
            TsconfigStrictState::ExplicitFalse,
        )?,
        strict_tsconfig_doc(
            "tsconfig.child_of_false.json",
            Some("./tsconfig.false_base.json"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc(
            "tsconfig.missing_parent.json",
            Some("./does_not_exist.json"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc(
            "tsconfig.cyc_a.json",
            Some("./tsconfig.cyc_b.json"),
            TsconfigStrictState::Omitted,
        )?,
        strict_tsconfig_doc(
            "tsconfig.cyc_b.json",
            Some("./tsconfig.cyc_a.json"),
            TsconfigStrictState::Omitted,
        )?,
    ];
    let bad_paths: Vec<String> = evaluate_workspace_sorted(
        &bad_docs,
        &ProjectContext::default(),
        &Config::strict_defaults(),
    )
    .into_iter()
    .filter(|d| d.rule_id == "GF081")
    .map(|d| String::from(d.path.as_str()))
    .collect();
    assert_eq!(
        bad_paths,
        vec![
            "tsconfig.child_of_false.json",
            "tsconfig.cyc_a.json",
            "tsconfig.cyc_b.json",
            "tsconfig.missing_parent.json",
        ]
    );
    Ok(())
}

fn sample_ts_alias_workspace() -> Result<(Vec<UastDocument>, Config), Box<dyn std::error::Error>> {
    let mut cfg = Config::strict_defaults();
    cfg.boundaries = vec![BoundaryRule {
        from: String::from("src/caller.ts"),
        allow: Vec::new(),
        deny: vec![
            String::from("packages/web/app/secret.ts"),
            String::from("src/exact/cfg.ts"),
            String::from("src/utils/math.ts"),
            String::from("src/both.js"),
            String::from("src/only_ts.ts"),
        ],
        reason: String::from("deny target"),
    }];
    let docs = vec![
        alias_tsconfig_doc(
            "packages/web/tsconfig.json",
            None,
            &[("@web/*", &["./app/*"])],
        )?,
        alias_tsconfig_doc(
            "tsconfig.json",
            Some("src"),
            &[("#cfg", &["./exact/cfg.ts"])],
        )?,
        ts_doc("packages/web/app/secret.ts", Vec::new())?,
        ts_doc("src/exact/cfg.ts", Vec::new())?,
        ts_doc("src/utils/math.ts", Vec::new())?,
        ts_doc("src/@scoped/pkg.ts", Vec::new())?,
        ts_doc("src/both.js", Vec::new())?,
        ts_doc("src/both.ts", Vec::new())?,
        ts_doc("src/only_ts.ts", Vec::new())?,
    ];
    Ok((docs, cfg))
}

#[test]
fn ts_import_resolver_handles_paths_wildcards_exact_aliases_baseurl_and_js_precedence() -> Res {
    let (base_docs, mut cfg) = sample_ts_alias_workspace()?;
    let ctx = ProjectContext::default();
    for blocked in [
        "@web/secret",
        "#cfg",
        "utils/math",
        "./both.js",
        "./only_ts.js",
    ] {
        let mut docs = base_docs.clone();
        docs.push(ts_doc("src/caller.ts", vec![imp(blocked)])?);
        assert!(
            evaluate_workspace_sorted(&docs, &ctx, &cfg)
                .iter()
                .any(|d| d.rule_id == "GF072")
        );
    }
    for allowed in ["#other", "@scoped/pkg", "app/secret", ""] {
        let mut docs = base_docs.clone();
        docs.push(ts_doc("src/caller.ts", vec![imp(allowed)])?);
        assert!(
            !evaluate_workspace_sorted(&docs, &ctx, &cfg)
                .iter()
                .any(|d| d.rule_id == "GF072")
        );
    }
    cfg.boundaries[0].deny = vec![String::from("src/both.ts")];
    let mut prec_docs = base_docs;
    prec_docs.push(ts_doc("src/caller.ts", vec![imp("./both.js")])?);
    assert!(
        !evaluate_workspace_sorted(&prec_docs, &ctx, &cfg)
            .iter()
            .any(|d| d.rule_id == "GF072")
    );
    Ok(())
}
