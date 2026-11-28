//! Boundary mutation guards for `toml_lang` (`mod.rs`, `manifests.rs`, `span_util.rs`)
//! and `markdown` (`mod.rs`, `frontmatter.rs`, `slugger_links.rs`, `mdx_bridge.rs`).

use gf_core::{Config, RepoPath};
use gf_parsers::langs::css::parser_ast::{CssRule, CssValue, parse_css_stylesheet};
use gf_parsers::langs::css::tokenizer::{CssTokKind, tokenize_css};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_msgs(path: &str, src: &str) -> Result<Vec<(u32, String)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| (d.span.start_line(), d.message))
        .collect())
}

#[test]
fn toml_arrays_aot_uv_pep621_mise_and_column_offsets_are_exact() -> Res {
    // 5-key inline table inside array + reopened `[[bin]]` AOT + clean standard tables
    let arr_toml = "items = [{ a = 1, b = 2, c = 3, d = 4, e = 5 }]\n";
    assert_eq!(eval_msgs("cfg.toml", arr_toml)?.len(), 1);

    let aot_reopen =
        "[[bin]]\nname = \"a\"\n[profile.release]\nlto = true\n[[bin]]\nname = \"b\"\n";
    assert_eq!(eval_msgs("Cargo.toml", aot_reopen)?.len(), 1);

    let std_tables = "[alpha.s1]\nx = 1\n[beta]\ny = 2\n[alpha.s2]\nz = 3\n";
    assert_eq!(eval_msgs("reopen.toml", std_tables)?.len(), 1);
    let nested_std = "[parent]\n[parent.a]\ny = 1\n[parent.b]\nz = 2\n[parent.a.deep]\nw = 3\n";
    assert_eq!(eval_msgs("ok.toml", nested_std)?, Vec::new());

    // Table-form wildcard in Cargo.toml + exact col offset on line 2 syntax error
    let cargo_tbl_wild = "[package]\nname = \"x\"\n[dependencies]\ns = { version = \"*\" }\nok = { version = \"1.0\" }\n";
    let cw_msgs = eval_msgs("Cargo.toml", cargo_tbl_wild)?;
    assert_eq!(cw_msgs.len(), 1);
    assert!(cw_msgs[0].1.contains("`s`"), "{cw_msgs:?}");

    let syn_doc = parse_source(&RepoPath::new("bad.toml")?, "ok = 1\nbad = @\n")?;
    assert_eq!(syn_doc.syntax_errors.len(), 1);
    assert_eq!(syn_doc.syntax_errors[0].span.start_line(), 2);
    assert!(
        syn_doc.syntax_errors[0].message.contains("col 7:"),
        "{:?}",
        syn_doc.syntax_errors[0].message
    );

    // pyproject.toml: `[tool.uv.sources]` (rev vs tag vs missing) & PEP 621 `@a` (< 2) vs `@ab` (>= 2)
    let pyproj = concat!(
        "[project]\n",
        "dependencies = [\n",
        "  \"u1 @ git+https://github.com/x/y\",\n",
        "  \"u2 @ git+https://github.com/x/y@a\",\n",
        "  \"ok @ git+https://github.com/x/y@ab\",\n",
        "]\n",
        "[tool.uv.sources]\n",
        "bad_uv = { git = \"https://github.com/x/y\" }\n",
        "ok_rev = { git = \"https://github.com/x/y\", rev = \"123456\" }\n",
        "ok_tag = { git = \"https://github.com/x/y\", tag = \"v1.0.0\" }\n",
    );
    let py_msgs = eval_msgs("pyproject.toml", pyproj)?;
    assert_eq!(py_msgs.len(), 3, "{py_msgs:?}");
    assert!(py_msgs.iter().any(|m| m.0 == 3), "{py_msgs:?}");
    assert!(py_msgs.iter().any(|m| m.0 == 4), "{py_msgs:?}");
    assert!(!py_msgs.iter().any(|m| m.0 == 5), "{py_msgs:?}");

    // `.mise.toml` inline-table and `[tools.<name>]` table `version = "latest"`
    let mise_src = "[tools]\nnode = { version = \"latest\" }\n[tools.python]\nversion = \"latest\"\n[tools.jq]\nversion = \"1.7.1\"\n";
    assert_eq!(eval_msgs(".mise.toml", mise_src)?.len(), 2);
    Ok(())
}

#[test]
fn markdown_headings_code_exclusions_schemes_and_images_are_exact() -> Res {
    assert_eq!(
        eval_msgs("docs/frag.md", "## Fragment Title\n\nBody.\n")?,
        Vec::new()
    );

    let clean_md = concat!(
        "# Title_One-Two\n\n",
        "## H2\n### H3\n#### H4\n##### H5\n###### H6\n## Back\n\n",
        "See [slug](#title_one-two), [src](./lib.rs), [a](http://x.io/a.md), [b](mailto:x@y.md), ",
        "[c](tel:+331.md), [d](file:///tmp/x.md), [e](//cdn.io/x.md), `code (rev)[https://x.io]`.\n\n",
        "```md\n(fenced_rev)[https://x.io]\n```\n\n",
        "![ext](https://cdn.io/a.png) ![frag](#back) ![md](./other.md) ![loc](../assets/ok.png?v=1)\n",
    );
    let doc = parse_source(&RepoPath::new("docs/clean.md")?, clean_md)?;
    assert_eq!(
        evaluate_document(&doc, &Config::strict_defaults()),
        Vec::new()
    );
    assert!(
        doc.contenus
            .as_ref()
            .is_some_and(|c| c.doc_links.is_empty())
    );
    assert_eq!(doc.asset_refs.len(), 1);
    assert!(doc.asset_refs.contains("../assets/ok.png"));

    let bad_md = concat!(
        "# First\n\n",
        "### Skipped H2\n\n",
        "# Second H1\n\n",
        "Links [e1]() and [e2](#) and ()[https://x.io] and `ok` (bad)[https://x.io] `ok2`.\n\n",
        "<!doctype html>\n\n",
        "![d](data:image/png;base64,AA)\n",
    );
    let msgs = eval_msgs("docs/bad_edges.md", bad_md)?;
    assert_eq!(msgs.len(), 6, "{msgs:?}");
    Ok(())
}

#[test]
fn mdx_fragments_jsx_bridges_imports_and_fenced_code_exclusions_are_exact() -> Res {
    let clean_mdx = concat!(
        "# Clean MDX\n\n",
        "import { Callout } from \"./components\";\n\n",
        "<>\n\n<Callout kind=\"note\">Hello</Callout>\n\n</>\n\n",
        "```tsx\nexport const demo = 1;\nimport \"./demo.css\";\nconst [a, b] = useState(0);\n```\n",
    );
    assert_eq!(eval_msgs("docs/clean.mdx", clean_mdx)?, Vec::new());

    let bad_mdx = concat!(
        "# Impure MDX\n\n",
        "import \"./leak.css\";\n\n",
        "<Card><div onClick={() => {}} /></Card>\n\n",
        "<Banner dangerouslySetInnerHTML={{ __html: raw }}>\n\n",
        "<Callout onClick=\"run()\" />\n\n",
        "<div>raw html in mdx</div>\n\n",
        "```tsx\nexport const insideFence = 1;\n```\n\n",
        "const [cnt, setCnt] = useState(0);\n",
    );
    let msgs = eval_msgs("docs/bad.mdx", bad_mdx)?;
    for expected_line in [3, 5, 7, 9, 11, 17] {
        assert!(
            msgs.iter().any(|m| m.0 == expected_line),
            "missing line {expected_line}: {msgs:?}"
        );
    }
    assert_eq!(msgs.iter().filter(|m| m.0 == 9).count(), 1, "{msgs:?}");
    assert!(
        !msgs.iter().any(|m| m.0 == 14),
        "inside fence leaked: {msgs:?}"
    );
    for (p, mdx) in [
        ("docs/one.mdx", "# T\n\n<Callout onClick=\"run()\" />\n"),
        (
            "docs/no_dup.mdx",
            "# T\n\n<Card><img src=\"data:image/png;base64,AA\" /></Card>\n",
        ),
        (
            "docs/unclosed.mdx",
            "# T\n\n<Banner style={{ color: \"red\" }}>\n",
        ),
        (
            "docs/danger1.mdx",
            "# T\n\n<Banner dangerouslySetInnerHTML={{ __html: raw }}>\n",
        ),
        ("docs/any_closed.mdx", "# T\n\n<Card>{1 as any}</Card>\n"),
    ] {
        assert_eq!(eval_msgs(p, mdx)?.len(), 1, "failed on {p}");
    }
    Ok(())
}

#[test]
fn css_tokenizer_preserves_exact_escapes_numbers_comments_and_strings() {
    let src = concat!(
        "/* stylelint-disable * hidden_blk_tok */ // hidden_line_tok\n",
        "keep//rest\n.cls_name\\:hover\\n",
        "\n{ width: .5em; scale: +.25 -.75 1.5 2.a +.a; content: \"a\\\"b\\\nc\"; }\n",
    );
    let out = tokenize_css(src);
    assert!(out.syntax_errors.is_empty(), "{:?}", out.syntax_errors);
    assert_eq!(out.bypasses.len(), 1);
    for hidden in ["hidden_blk_tok", "hidden_line_tok"] {
        assert!(
            !out.tokens
                .iter()
                .any(|t| t.kind == CssTokKind::Ident(hidden.into()))
        );
    }
    for expected in [
        CssTokKind::Ident("keep".into()),
        CssTokKind::Ident("cls_name\\:hover\\n".into()),
        CssTokKind::Dimension {
            num: ".5".into(),
            unit: "em".into(),
        },
        CssTokKind::Number("+.25".into()),
        CssTokKind::Number("-.75".into()),
        CssTokKind::Number("2".into()),
        CssTokKind::Delim('+'),
        CssTokKind::Str("a\"bc".into()),
    ] {
        assert!(
            out.tokens.iter().any(|t| t.kind == expected),
            "missing {expected:?}"
        );
    }
}

#[test]
fn css_ast_parser_distinguishes_nested_selectors_functions_and_at_rules() {
    let src = concat!(
        "} ; @import \"./base.css\"; .outer { @layer base }\n",
        ".card { ; color: var(--fg, fn({a:1})) ; @media (min-width: 1px) { span:hover { opacity: 1 } } ",
        "a:not(.x) { outline: none } a:has(;) { fill: none } stray }\n",
    );
    let rules = parse_css_stylesheet(&tokenize_css(src).tokens);
    assert_eq!(rules.len(), 3, "{rules:?}");
    assert!(
        matches!(&rules[0], CssRule::AtRule { name, declarations, .. } if name == "import" && declarations.is_empty())
    );
    let CssRule::QualifiedRule {
        declarations,
        nested_rules,
        ..
    } = &rules[2]
    else {
        return assert_eq!(rules.len(), 0, "expected QualifiedRule");
    };
    assert_eq!(declarations.len(), 1, "{declarations:?}");
    assert_eq!(declarations[0].property, "color");
    assert!(
        matches!(declarations[0].values.first(), Some(CssValue::Function { name, .. }) if name == "var")
    );
    assert_eq!(nested_rules.len(), 4, "{nested_rules:?}");
    assert!(
        matches!(&nested_rules[0], CssRule::AtRule { name, nested_rules: inner, .. } if name == "media" && inner.len() == 1)
    );
    assert!(
        matches!(&nested_rules[1], CssRule::QualifiedRule { declarations: d, .. } if d.len() == 1 && d[0].property == "outline")
    );
    assert!(
        matches!(&nested_rules[2], CssRule::QualifiedRule { declarations: d, .. } if d.len() == 1 && d[0].property == "fill")
    );
    assert!(
        matches!(&nested_rules[3], CssRule::QualifiedRule { declarations: d, .. } if d.is_empty())
    );
}
