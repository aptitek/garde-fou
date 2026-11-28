//! Boundary mutation guards for CSS (`colors`, `tokenizer`, `parser_ast`, `value_rules`,
//! `svg_css_bridge`), XML/SVG (`xxe_dtd`, `svg_security`, `xml_manifests`), and Tier 2/3 parsers.

use std::path::Path;

use gf_core::{Config, ExternalToolKind, RepoPath};
use gf_parsers::langs::css::colors::{
    is_css_named_color, is_direct_color_function, is_intrinsic_color_keyword, is_valid_hex_color,
};
use gf_parsers::{
    parse_rumdl_or_markdownlint_json, parse_source, parse_taplo_output, parse_xmllint_output,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq, Eq)]
struct DiagMsg {
    line: u32,
    message: String,
}

fn eval_msgs(path: &str, src: &str) -> Result<Vec<DiagMsg>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| DiagMsg {
            line: d.span.start_line(),
            message: d.message,
        })
        .collect())
}

#[test]
fn css_colors_hex_lengths_keywords_functions_and_token_delimiters_are_exact() -> Res {
    // Intrinsic keywords & rem units & font-family color-like names -> 0 violations
    let clean = concat!(
        "a[data-state=\"open\"] {\n",
        "  color: inherit;\n",
        "  border-color: revert-layer;\n",
        "  padding: .5rem +.25rem -0.5rem 1rem;\n",
        "  font-family: Georgia, coral;\n",
        "  --icône_1: var(--fg);\n",
        "  content: \"/\";\n",
        "}\n",
    );
    assert_eq!(eval_msgs("styles/clean.css", clean)?.len(), 0);

    assert!(is_intrinsic_color_keyword("currentColor"));
    assert!(is_intrinsic_color_keyword("revert-layer"));
    assert!(!is_intrinsic_color_keyword("rebeccapurple"));
    assert!(is_css_named_color("RebeccaPurple"));
    assert!(is_direct_color_function("oklch") && !is_direct_color_function("calc"));
    assert!(
        is_valid_hex_color("1a2b3c") && !is_valid_hex_color("1a2b3g") && !is_valid_hex_color("12")
    );

    // 3, 4, 6, 8 hex digits + oklch() + rgb() + important with 5 tokens + translatable content
    let bad_colors = concat!(
        ".c {\n",
        "  a: #abc;\n",
        "  b: #abcd;\n",
        "  c: #112233;\n",
        "  d: #11223344;\n",
        "  e: oklch(0.6 0.1 20);\n",
        "  f: rgb(255 0 0);\n",
        "  g: #12 #12345 #gggggg;\n",
        "  padding: 0 0 0 12pt !important;\n",
        "  content: \"Bonjour\";\n",
        "}\n",
    );
    let msgs = eval_msgs("styles/bad.css", bad_colors)?;
    assert_eq!(msgs.len(), 9, "{msgs:?}");
    Ok(())
}

#[test]
fn css_delimiters_charset_escaped_strings_and_selector_lookahead_are_exact() -> Res {
    assert_eq!(
        eval_msgs("styles/unclosed_br.css", "a[href { color: var(--fg); }\n")?.len(),
        1
    );
    assert_eq!(
        eval_msgs("styles/mismatch_br.css", "a[href)] { color: var(--fg); }\n")?.len(),
        2
    );
    let tricky = concat!(
        "@charset \"utf-8\";\n",
        "/* star * inside -- 0123456789 */\n",
        "// line comment before rule\n",
        "@layer base {\n",
        "  a:has(> [data-x]) + +.a, _u { width: calc(100% / 2); --v: 10.foo; content: \"a\\\nb\"; color: red }\n",
        "  orphan_prelude }\n",
    );
    let tricky_msgs = eval_msgs("styles/tricky.css", tricky)?;
    assert_eq!(tricky_msgs.len(), 2, "{tricky_msgs:?}");
    Ok(())
}

#[test]
fn css_urls_asset_refs_and_declaration_lookahead_boundaries() -> Res {
    let css = concat!(
        ".bg {\n",
        "  background-image: url(  ../assets/bg.svg?v=1#top  );\n",
        "  mask-image: url(\"\"), url(\"https://cdn.io/x.svg\"), url(\"//cdn.io/y.svg\"), url(\"javascript:x\"), url(\"#id\");\n",
        "  cursor: url(data:image/png;base64,AAAA), pointer;\n",
        "  nested:hover { color: red; }\n",
        "}\n",
    );
    let doc = parse_source(&RepoPath::new("styles/urls.css")?, css)?;
    assert_eq!(doc.asset_refs.len(), 1);
    assert!(doc.asset_refs.contains("../assets/bg.svg"));
    let diags = evaluate_document(&doc, &Config::strict_defaults());
    // `data:` URI on line 4 + `red` inside `nested:hover` on line 5
    assert_eq!(diags.len(), 2, "{diags:?}");
    Ok(())
}

#[test]
fn xml_xxe_comments_nodes_limit_svg_smil_and_manifests_boundaries() -> Res {
    // Commented DTD on line 1 ignored; PUBLIC DTD on line 2 flagged at line 2; plain DOCTYPE & SYSTEMATIC ok
    let pub_dtd = "<!--\n<!ENTITY ignored \"1\">\n<!DOCTYPE x SYSTEM \"http://x\">\n-->\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://x\">\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n";
    let dtd_msgs = eval_msgs("assets/pub.svg", pub_dtd)?;
    assert_eq!(dtd_msgs.len(), 1, "{dtd_msgs:?}");
    assert_eq!(dtd_msgs[0].line, 5);
    assert_eq!(
        eval_msgs(
            "assets/plain.svg",
            "<!DOCTYPE svg><svg xmlns=\"http://www.w3.org/2000/svg\"/>\n"
        )?
        .len(),
        0
    );

    // Billion-Laughs / excessive node count (> 32,768 nodes)
    let huge = format!("<svg>{}</svg>", "<g/>".repeat(33_000));
    let huge_msgs = eval_msgs("assets/huge.svg", &huge)?;
    assert!(
        huge_msgs
            .iter()
            .any(|m| m.message.contains("[XML-FORBID-DTD-XXE]")),
        "{huge_msgs:?}"
    );

    // Safe SVG attributes (`on` length 2, `<a href="https://...">`, `<use href="#id">`, safe `<animate>`)
    let safe_svg = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\">\n",
        "  <a href=\"https://example.org\"><rect on=\"1\" style=\"fill: url(#grad); ; \"/></a>\n",
        "  <use href=\"#local\"/>\n",
        "  <animate attributeName=\"opacity\" values=\"0;1\"/>\n",
        "  <style>@media screen { .ok { fill: currentColor; } }</style>\n",
        "</svg>\n",
    );
    assert_eq!(eval_msgs("assets/safe.svg", safe_svg)?.len(), 0);

    // Unsafe SVG: control-char URI, data: href, SMIL xlink:href, multi-node <style>
    let unsafe_svg = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">\n",
        "  <a xlink:href=\"java&#x7f;script:evil()\"/>\n",
        "  <image href=\"data:image/png;base64,AA\"/>\n",
        "  <animate attributeName=\"xlink:href\" values=\"0;javascript:alert(1)\"/>\n",
        "  <rect onclick=\"run()\" style=\"fill: url(https://evil.io/x.svg);\"/>\n",
        "  <style>.ok { fill: none; }\n<![CDATA[\n@import \"https://evil.io/a.css\";\n]]></style>\n",
        "</svg>\n",
    );
    let us_msgs = eval_msgs("assets/unsafe.svg", unsafe_svg)?;
    assert_eq!(us_msgs.len(), 6, "{us_msgs:?}");
    assert!(us_msgs.iter().any(|m| m.line == 8), "{us_msgs:?}");
    Ok(())
}

#[test]
fn xml_maven_msbuild_and_tier2_tier3_edge_boundaries() -> Res {
    let clean_pom = "<project><dependencies><dependency><artifactId>a</artifactId><version>1.2.3</version></dependency></dependencies><repositories><repository><url>https://repo.maven.apache.org/maven2</url></repository></repositories></project>";
    assert_eq!(eval_msgs("pom.xml", clean_pom)?.len(), 0);

    let bad_pom = "<project><build><plugins><plugin><artifactId>p</artifactId><version>*</version></plugin></plugins></build><pluginRepositories><pluginRepository><url>http://insecure/m2</url></pluginRepository></pluginRepositories><dependencies><dependency><artifactId>d</artifactId><version>LATEST</version></dependency></dependencies></project>";
    assert_eq!(eval_msgs("pom.xml", bad_pom)?.len(), 3);

    let bad_props = "<Project><ItemGroup><PackageReference Include=\"PkgInc\" Version=\"*\" /><PackageVersion Update=\"Serilog\"><Version>*</Version></PackageVersion></ItemGroup></Project>";
    let prop_msgs = eval_msgs("Directory.Packages.props", bad_props)?;
    assert_eq!(prop_msgs.len(), 2);
    assert!(
        prop_msgs.iter().any(|m| m.message.contains("PkgInc")),
        "{prop_msgs:?}"
    );
    assert!(
        prop_msgs.iter().any(|m| m.message.contains("Serilog")),
        "{prop_msgs:?}"
    );

    // `.stylelintrc` (no `.json` extension) with JSONC comment + `svgo.config.mjs` on line 3
    let sl_jsonc = "{\n  // comment\n  \"rules\": { \"color-no-hex\": null }\n}\n";
    assert_eq!(eval_msgs(".stylelintrc", sl_jsonc)?.len(), 1);
    let svgo_l3 = "export default {\n  plugins: [\n    { removeScripts: false }\n  ]\n};\n";
    let svgo_msgs = eval_msgs("svgo.config.mjs", svgo_l3)?;
    assert_eq!(svgo_msgs.len(), 1);
    assert_eq!(svgo_msgs[0].line, 3);

    // Rumdl vs markdownlint single-field discriminators & short relative taplo/xmllint paths
    let root = Path::new("/repo");
    let md = parse_rumdl_or_markdownlint_json(
        "[{\"file\":\"docs/a.md\",\"line\":2},{\"fileName\":\"docs/b.md\",\"rule\":\"MD001\",\"lineNumber\":3}]",
        root,
    )?;
    assert_eq!(md[0].tool, ExternalToolKind::Rumdl);
    assert_eq!(md[1].tool, ExternalToolKind::Rumdl);
    let tp = parse_taplo_output(
        "error: bad\n  --> C:/repo/a.toml:7:2\nerror: n\n  --> 1:/7:3\n  --> x\n",
        Path::new("C:/repo"),
    )?;
    assert_eq!(tp.len(), 2);
    assert!(
        tp.iter()
            .any(|f| f.path.as_str() == "a.toml" && f.line == 7)
    );
    assert!(tp.iter().any(|f| f.path.as_str() == "1" && f.line == 1));
    let xl = parse_xmllint_output(
        "a\nC:\\repo\\b.xml:5: parser error : oops\nab\n",
        Path::new("C:\\repo"),
    )?;
    assert_eq!(xl.len(), 1);
    assert_eq!(xl[0].line, 5);
    Ok(())
}
