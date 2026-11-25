//! Unit & boundary mutation tests for `gf-rules::format_workspace_checks`
//! (`TOML-WORKSPACE-BYPASS` -> `GF080` and `MD-UNRESOLVED-REF-OR-ANCHOR` -> `GF063`).

use std::collections::BTreeSet;

use gf_core::{
    Config, ContenusDocument, Language, MarkdownLinkTarget, ProjectContext, RepoPath, Span,
    TomlManifestSummary, UastDocument,
};
use gf_rules::evaluate_workspace_sorted;

type Res = Result<(), Box<dyn std::error::Error>>;

fn make_toml_doc(
    path: &str,
    manifest: TomlManifestSummary,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Toml, 5, 5)?;
    doc.toml_manifest = Some(manifest);
    Ok(doc)
}

fn make_md_doc(
    path: &str,
    slugs: &[&str],
    links: Vec<MarkdownLinkTarget>,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Markdown, 5, 5)?;
    doc.contenus = Some(ContenusDocument {
        issues: Vec::new(),
        heading_slugs: slugs.iter().map(|s| String::from(*s)).collect(),
        doc_links: links,
    });
    Ok(doc)
}

#[test]
fn cargo_workspace_bypass_ignores_non_workspace_and_empty_roots() -> Res {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();
    let non_ws_root = make_toml_doc(
        "Cargo.toml",
        TomlManifestSummary {
            is_workspace_root: false,
            workspace_deps: BTreeSet::from([String::from("serde")]),
            hardcoded_member_deps: Vec::new(),
        },
    )?;
    let member = make_toml_doc(
        "crates/sub/Cargo.toml",
        TomlManifestSummary {
            is_workspace_root: false,
            workspace_deps: BTreeSet::new(),
            hardcoded_member_deps: vec![(String::from("serde"), Span::FIRST)],
        },
    )?;
    assert_eq!(
        evaluate_workspace_sorted(&[non_ws_root, member.clone()], &ctx, &cfg),
        Vec::new()
    );
    let empty_ws_root = make_toml_doc(
        "Cargo.toml",
        TomlManifestSummary {
            is_workspace_root: true,
            workspace_deps: BTreeSet::new(),
            hardcoded_member_deps: Vec::new(),
        },
    )?;
    assert_eq!(
        evaluate_workspace_sorted(&[empty_ws_root, member], &ctx, &cfg),
        Vec::new()
    );
    Ok(())
}

#[test]
fn cargo_workspace_bypass_flags_member_and_skips_root_and_non_cargo() -> Res {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();
    let ws_root = make_toml_doc(
        "Cargo.toml",
        TomlManifestSummary {
            is_workspace_root: true,
            workspace_deps: BTreeSet::from([String::from("serde")]),
            hardcoded_member_deps: vec![(String::from("serde"), Span::FIRST)],
        },
    )?;
    let non_cargo = make_toml_doc(
        "mise.toml",
        TomlManifestSummary {
            is_workspace_root: false,
            workspace_deps: BTreeSet::new(),
            hardcoded_member_deps: vec![(String::from("serde"), Span::FIRST)],
        },
    )?;
    let member = make_toml_doc(
        "crates/sub/Cargo.toml",
        TomlManifestSummary {
            is_workspace_root: false,
            workspace_deps: BTreeSet::new(),
            hardcoded_member_deps: vec![(String::from("serde"), Span::FIRST)],
        },
    )?;
    let diags = evaluate_workspace_sorted(&[ws_root, non_cargo, member], &ctx, &cfg);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, "GF080");
    assert_eq!(diags[0].path.as_str(), "crates/sub/Cargo.toml");
    Ok(())
}

#[test]
fn markdown_cross_links_resolve_root_parent_dotdot_and_fragment_boundaries() -> Res {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();

    let root_md = make_md_doc(
        "README.md",
        &["top"],
        vec![
            MarkdownLinkTarget {
                target_path: String::from("./docs/guide.md"),
                fragment: Some(String::from("intro")),
                span: Span::FIRST,
            },
            MarkdownLinkTarget {
                target_path: String::from("docs/guide.md"),
                fragment: Some(String::new()),
                span: Span::FIRST,
            },
        ],
    )?;
    let guide_md = make_md_doc(
        "docs/sub/nested.md",
        &["nested"],
        vec![
            MarkdownLinkTarget {
                target_path: String::from("../guide.md"),
                fragment: Some(String::from("intro")),
                span: Span::FIRST,
            },
            MarkdownLinkTarget {
                target_path: String::from("/docs/guide.md"),
                fragment: Some(String::from("missing-slug")),
                span: Span::FIRST,
            },
            MarkdownLinkTarget {
                target_path: String::from("./absent.md"),
                fragment: None,
                span: Span::FIRST,
            },
        ],
    )?;
    let target_md = make_md_doc("docs/guide.md", &["intro"], Vec::new())?;

    let diags = evaluate_workspace_sorted(&[root_md, guide_md, target_md], &ctx, &cfg);
    assert_eq!(diags.len(), 2, "{diags:?}");
    assert!(
        diags
            .iter()
            .all(|d| d.rule_id == "GF063" && d.path.as_str() == "docs/sub/nested.md")
    );
    Ok(())
}
