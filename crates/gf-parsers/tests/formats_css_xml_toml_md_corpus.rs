//! Symmetric specification corpus tests (`(invalid, subtle_valid)` with exact 1-based lines)
//! across CSS (`Forme`), XML & SVG (`Asset` / `Meta`), TOML (`Meta` / `Texte`), and Markdown/MDX (`Contenus`).

use gf_core::{Config, ProjectContext, RepoPath};
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_single(path: &str, src: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| format!("{}:{}:{}", d.rule_id, d.span.start_line(), d.message))
        .collect())
}

fn eval_multi(files: &[(&str, &str)]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let cfg = Config::strict_defaults();
    let mut docs = Vec::new();
    let mut out = Vec::new();
    for (p, s) in files {
        let doc = parse_source(&RepoPath::new(p)?, s)?;
        for d in evaluate_document(&doc, &cfg) {
            out.push(format!(
                "{}:{}:{}:{}",
                d.rule_id,
                d.path,
                d.span.start_line(),
                d.message
            ));
        }
        docs.push(doc);
    }
    for d in evaluate_workspace_sorted(&docs, &ProjectContext::default(), &cfg) {
        out.push(format!(
            "{}:{}:{}:{}",
            d.rule_id,
            d.path,
            d.span.start_line(),
            d.message
        ));
    }
    Ok(out)
}

fn has_at(items: &[String], needle: &str, line: u32) -> bool {
    let tag = format!(":{line}:");
    items.iter().any(|m| m.contains(needle) && m.contains(&tag))
}

#[test]
fn css_w3c_named_colors_fallbacks_calc_selectors_and_tokens_are_exact() -> Res {
    let bad_css = concat!(
        "@import url(\"./reset.css\");\n",
        "#hero {\n",
        "  color: cornflowerblue;\n",
        "  background: var(--bg, #fff);\n",
        "  width: calc(100% - 12px);\n",
        "  z-index: 999;\n",
        "  transition: all 0.2s !important;\n",
        "}\n",
        "/* stylelint-disable */\n",
    );
    let diags = eval_single("styles/hero.css", bad_css)?;
    for (code, line) in [
        ("GF061", 1),
        ("[CSS-NO-AT-IMPORT]", 1),
        ("[CSS-NO-ID-SELECTOR]", 2),
        ("[CSS-RAW-COLOR]", 3),
        ("[CSS-FALLBACK-TOKEN-LEAK]", 4),
        ("[CSS-RAW-DIMENSION-OR-ZINDEX]", 5),
        ("[CSS-RAW-DIMENSION-OR-ZINDEX]", 6),
        ("[CSS-NO-TRANSITION-ALL]", 7),
        ("[CSS-NO-IMPORTANT]", 7),
        ("GF070", 9),
    ] {
        assert!(
            has_at(&diags, code, line),
            "missing {code} at line {line}: {diags:?}"
        );
    }

    let clean_css = concat!(
        ".hero {\n",
        "  color: var(--fg, currentColor);\n",
        "  background: transparent;\n",
        "  margin: 0;\n",
        "  z-index: var(--z-modal);\n",
        "  transition: opacity var(--dur-fast);\n",
        "}\n",
    );
    assert_eq!(
        eval_single("styles/clean.css", clean_css)?,
        Vec::<String>::new()
    );

    let token_css = ":root {\n  --solarized-bg: #002b36;\n  --space-md: 16px;\n}\n";
    assert_eq!(
        eval_single("styles/tokens/palette.css", token_css)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn xml_xxe_svg_entity_decoded_smil_css_bridge_and_manifests_are_exact() -> Res {
    let bad_svg = concat!(
        "<!DOCTYPE svg [ <!ENTITY x \"y\"> ]>\n",
        "<svg xmlns=\"http://www.w3.org/2000/svg\">\n",
        "  <a href=\"&#x6a;&#x61;vascript:alert(1)\"/>\n",
        "  <set attributeName=\"onload\" to=\"evil()\"/>\n",
        "  <foreignObject width=\"10\" height=\"10\"/>\n",
        "  <use href=\"https://evil.example/sprite.svg#ic\"/>\n",
        "  <style>@import url(\"https://evil.example/x.css\");</style>\n",
        "</svg>\n",
    );
    let svg_diags = eval_single("assets/bad.svg", bad_svg)?;
    for (code, line) in [
        ("[XML-FORBID-DTD-XXE]", 1),
        ("[SVG-UNSAFE-HREF-OR-USE]", 3),
        ("[SVG-EVENT-OR-SMIL-XSS]", 4),
        ("[SVG-DANGEROUS-ELEMENT]", 5),
        ("[SVG-UNSAFE-HREF-OR-USE]", 6),
        ("[SVG-UNSAFE-HREF-OR-USE]", 7),
    ] {
        assert!(
            has_at(&svg_diags, code, line),
            "missing {code}@{line}: {svg_diags:?}"
        );
    }

    let clean_svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"currentColor\" d=\"M0 0h24v24H0z\"/></svg>\n";
    assert_eq!(
        eval_single("assets/icon.svg", clean_svg)?,
        Vec::<String>::new()
    );

    let bad_csproj = "<Project><ItemGroup>\n  <PackageReference Include=\"Newtonsoft.Json\" Version=\"13.*\" />\n</ItemGroup></Project>\n";
    let cs_diags = eval_single("App.csproj", bad_csproj)?;
    assert!(has_at(&cs_diags, "[XML-UNPINNED-DEPENDENCY]", 2));

    let clean_csproj = "<Project><ItemGroup>\n  <PackageReference Include=\"Newtonsoft.Json\" Version=\"13.0.3\" />\n</ItemGroup></Project>\n";
    assert_eq!(
        eval_single("App.csproj", clean_csproj)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn toml_table_reopen_git_pins_mise_latest_and_workspace_bypass_are_exact() -> Res {
    let bad_ws = eval_multi(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/sub\"]\n[workspace.dependencies]\nserde = \"1.0\"\nfoo = { git = \"https://github.com/x/y\" }\nbar = \"*\"\n",
        ),
        (
            "crates/sub/Cargo.toml",
            "[package]\nname = \"sub\"\nversion = \"0.1.0\"\n[dependencies]\nserde = \"1.0\"\n",
        ),
        ("mise.toml", "[tools]\njq = \"latest\"\n"),
        (
            "bad_reopen.toml",
            "[alpha]\nx = 1\n[beta]\ny = 2\n[alpha.nested]\nz = 3\n",
        ),
    ])?;
    for (code, line) in [
        ("[TOML-UNPINNED-GIT-OR-WILDCARD]", 5),
        ("[TOML-UNPINNED-GIT-OR-WILDCARD]", 6),
        ("[TOML-WORKSPACE-BYPASS]", 5),
        ("[TOML-UNPINNED-GIT-OR-WILDCARD]", 2),
        ("[TOML-CONFUSING-TABLE-REOPEN]", 5),
    ] {
        assert!(
            has_at(&bad_ws, code, line),
            "missing {code}@{line}: {bad_ws:?}"
        );
    }

    let clean_ws = eval_multi(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/sub\"]\n[workspace.dependencies]\nserde = \"1.0\"\nfoo = { git = \"https://github.com/x/y\", rev = \"a1b2c3d4e5f6\" }\n",
        ),
        (
            "crates/sub/Cargo.toml",
            "[package]\nname = \"sub\"\nversion = \"0.1.0\"\n[dependencies]\nserde = { workspace = true }\n",
        ),
        ("mise.toml", "[tools]\njq = \"1.7.1\"\n"),
    ])?;
    assert_eq!(clean_ws, Vec::<String>::new());
    Ok(())
}

#[test]
fn markdown_autolinks_nested_fences_frontmatter_slugs_and_mdx_are_exact() -> Res {
    let clean_md = concat!(
        "---\ntitle: Guide\n---\n",
        "# Architecture & Design\n\n",
        "Visit <https://github.com/aptitek/garde-fou> or [Section](#architecture--design).\n\n",
        "````md\n```html\n<div>example inside nested fence</div>\n```\n````\n",
    );
    assert_eq!(
        eval_single("docs/guide.md", clean_md)?,
        Vec::<String>::new()
    );

    let bad_docs = eval_multi(&[
        (
            "docs/bad.md",
            concat!(
                "---\ndup: 1\ndup: 2\n---\n",
                "# Intro\n",
                "Inline <span>brut</span> here.\n",
                "See (reversed)[https://x.io] and [ghost](#ancre-fantome) and [ref][missing].\n",
                "Cross [other](./doc.md#absente).\n",
            ),
        ),
        ("docs/doc.md", "# Present Heading\n\n## Details\n"),
        (
            "docs/page.mdx",
            "# Page\n\n<Callout kind=\"info\" style={{ color: \"#ff0000\" }} />\n\nexport const x = 1;\n",
        ),
    ])?;
    for (code, line) in [
        ("[YAML-DUPLICATE-KEY]", 3),
        ("[MD-RAW-HTML]", 6),
        ("[MD-EMPTY-OR-REVERSED-LINK]", 7),
        ("[MD-UNRESOLVED-REF-OR-ANCHOR]", 7),
        ("[MD-UNRESOLVED-REF-OR-ANCHOR]", 8),
        ("[MD-FORBIDDEN-MDX-LOGIC]", 3),
        ("[MD-FORBIDDEN-MDX-LOGIC]", 5),
    ] {
        assert!(
            has_at(&bad_docs, code, line),
            "missing {code}@{line}: {bad_docs:?}"
        );
    }
    Ok(())
}
