//! Direct unit tests in `gf-rules` for `domain_checks.rs` (`GF060`–`GF064`, `GF066`).

use std::collections::BTreeSet;

use gf_core::{
    AssetDocument, AssetHazard, AssetHazardKind, Config, ContenusDocument, ContenusIssue,
    ContenusIssueKind, FormeDocument, FormeFinding, FormeFindingKind, Language, RepoPath, Span,
    StyleLeakFinding, StyleLeakKind, TexteDocument, TexteIssue, UastDocument, UiCheckFinding,
    UiCheckKind,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn doc_at(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 8)?)
}

fn ff(kind: FormeFindingKind, detail: &str) -> FormeFinding {
    FormeFinding {
        kind,
        detail: detail.into(),
        span: Span::FIRST,
    }
}

#[test]
fn code_and_forme_domains_enforce_style_a11y_and_token_boundaries() -> Res {
    let cfg = Config::strict_defaults();
    assert_eq!(
        evaluate_document(&doc_at("src/leak.css", Language::Css)?, &cfg)[0].rule_id,
        "GF060"
    );
    assert_eq!(
        evaluate_document(&doc_at("styles/ok.css", Language::Css)?, &cfg),
        Vec::new()
    );

    let mut tsx = doc_at("src/App.tsx", Language::Tsx)?;
    tsx.style_leaks = vec![
        StyleLeakFinding {
            kind: StyleLeakKind::InlineStyleAttr,
            detail: "s".into(),
            span: Span::FIRST,
        },
        StyleLeakFinding {
            kind: StyleLeakKind::InlineSvg,
            detail: "svg".into(),
            span: Span::FIRST,
        },
    ];
    tsx.ui_checks.push(UiCheckFinding {
        kind: UiCheckKind::ImgMissingAlt,
        detail: "img".into(),
        span: Span::FIRST,
    });
    let tsx_diags = evaluate_document(&tsx, &cfg);
    assert!(tsx_diags.iter().any(|d| d.rule_id == "GF060"));
    assert!(tsx_diags.iter().any(|d| d.rule_id == "GF064"));
    assert!(tsx_diags.iter().any(|d| d.rule_id == "GF066"));

    let findings = vec![
        ff(FormeFindingKind::RawColor, "#fff"),
        ff(FormeFindingKind::TranslatedContentText, "Hi"),
        ff(FormeFindingKind::DataUri, "data:"),
    ];
    let mut style_doc = doc_at("styles/card.css", Language::Css)?;
    style_doc.forme = Some(FormeDocument {
        findings: findings.clone(),
    });
    assert_eq!(evaluate_document(&style_doc, &cfg).len(), 3);
    let mut token_doc = doc_at("tokens/colors.css", Language::Css)?;
    token_doc.forme = Some(FormeDocument { findings });
    assert_eq!(evaluate_document(&token_doc, &cfg).len(), 2);
    Ok(())
}

#[test]
fn contenus_and_texte_enforce_markdown_and_i18n_purity() -> Res {
    let cfg = Config::strict_defaults();
    let mut locale = doc_at("locales/en.json", Language::Json)?;
    locale.texte = Some(TexteDocument {
        locale: "en".into(),
        keys: BTreeSet::new(),
        forbidden_values: vec![TexteIssue {
            key: "k".into(),
            detail: "<b>".into(),
            span: Span::FIRST,
        }],
        ..TexteDocument::default()
    });
    assert_eq!(evaluate_document(&locale, &cfg)[0].rule_id, "GF062");

    assert_eq!(
        evaluate_document(&doc_at("src/notes.md", Language::Markdown)?, &cfg)[0].rule_id,
        "GF063"
    );
    for clean_md in ["docs/guide.md", "tests/fix.md", "README.md"] {
        assert_eq!(
            evaluate_document(&doc_at(clean_md, Language::Markdown)?, &cfg),
            Vec::new()
        );
    }

    let mut md_uri = doc_at("docs/uri.md", Language::Markdown)?;
    md_uri.contenus = Some(ContenusDocument {
        issues: vec![ContenusIssue {
            kind: ContenusIssueKind::DataUri,
            detail: "data:".into(),
            span: Span::FIRST,
        }],
        ..ContenusDocument::default()
    });
    assert_eq!(evaluate_document(&md_uri, &cfg)[0].rule_id, "GF064");
    let mut md_html = doc_at("docs/html.md", Language::Markdown)?;
    md_html.contenus = Some(ContenusDocument {
        issues: vec![ContenusIssue {
            kind: ContenusIssueKind::RawHtml,
            detail: "<div>".into(),
            span: Span::FIRST,
        }],
        ..ContenusDocument::default()
    });
    assert_eq!(evaluate_document(&md_html, &cfg)[0].rule_id, "GF063");
    Ok(())
}

#[test]
fn local_asset_enforces_domain_byte_limits_and_svg_hazards() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.domains.max_asset_bytes = 2048;

    let mut exact_asset = doc_at("assets/ok.png", Language::BinaryAsset)?;
    exact_asset.asset = Some(AssetDocument {
        byte_size: 2048,
        extension: "png".into(),
        hazards: Vec::new(),
    });
    assert_eq!(evaluate_document(&exact_asset, &cfg), Vec::new());

    let mut bad_asset = doc_at("src/bad.svg", Language::Svg)?;
    bad_asset.asset = Some(AssetDocument {
        byte_size: 3072,
        extension: "svg".into(),
        hazards: vec![AssetHazard {
            kind: AssetHazardKind::SvgScriptTag,
            detail: "<script>".into(),
            span: Span::FIRST,
        }],
    });
    let ad = evaluate_document(&bad_asset, &cfg);
    assert_eq!(ad.len(), 3);
    assert!(ad.iter().any(|d| d.measured == 3 && d.threshold == 2));
    Ok(())
}
