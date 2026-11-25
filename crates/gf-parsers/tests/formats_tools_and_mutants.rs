//! Tier 2 (`GF081`), Tier 3 (`GF082`), and boundary mutation tests across CSS, XML/SVG,
//! TOML, and Markdown/MDX analyzers.

use std::path::Path;

use gf_core::{Config, ExternalToolKind, RepoPath};
use gf_parsers::{
    parse_rumdl_or_markdownlint_json, parse_source, parse_stylelint_json, parse_taplo_output,
    parse_xmllint_output,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn count_diags(path: &str, src: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults()).len())
}

#[test]
fn tier2_stylelint_and_markdown_configs_clean_vs_disabled_boundaries() -> Res {
    assert_eq!(
        count_diags(".stylelintrc.json", "{\"rules\":{\"color-no-hex\":true}}")?,
        0
    );
    assert_eq!(
        count_diags(".stylelintrc.json", "{\"rules\":{\"color-no-hex\":null}}")?,
        1
    );
    assert_eq!(
        count_diags(
            ".stylelintrc.yml",
            "rules:\n  declaration-no-important: [false]\n"
        )?,
        1
    );
    assert_eq!(
        count_diags(".stylelintrc.yml", "rules:\n  selector-max-id: 0\n")?,
        1
    );

    assert_eq!(
        count_diags(".markdownlint.json", "{\"default\":true,\"MD033\":true}")?,
        0
    );
    assert_eq!(
        count_diags(".markdownlint.json", "{\"default\":false,\"MD033\":false}")?,
        2
    );
    assert_eq!(
        count_diags("rumdl.toml", "[global]\ndisable = [\"MD013\"]\n")?,
        0
    );
    assert_eq!(
        count_diags(
            "rumdl.toml",
            "[global]\ndisable = [\"MD001\", \"no-inline-html\"]\n"
        )?,
        2
    );
    Ok(())
}

#[test]
fn tier2_taplo_and_svgo_configs_clean_vs_disabled_boundaries() -> Res {
    assert_eq!(
        count_diags("taplo.toml", "exclude = [\"vendor/*.toml\"]\n")?,
        0
    );
    assert_eq!(count_diags("taplo.toml", "exclude = [\"**/*.toml\"]\n")?, 1);

    assert_eq!(
        count_diags(
            "svgo.config.mjs",
            "export default { plugins: [{ removeScripts: true }] };\n"
        )?,
        0
    );
    assert_eq!(
        count_diags(
            "svgo.config.mjs",
            "export default { plugins: [{ removeScripts: false }] };\n"
        )?,
        1
    );
    Ok(())
}

#[test]
fn tier3_windows_safe_stylelint_rumdl_markdownlint_taplo_and_xmllint_parsers() -> Res {
    let root = Path::new("/repo");
    assert_eq!(parse_stylelint_json("   ", root)?.len(), 0);
    assert!(parse_stylelint_json("{broken", root).is_err());
    let sl = parse_stylelint_json(
        "[{\"source\":\"/repo/styles/a.css\",\"warnings\":[{\"line\":4,\"column\":3,\"rule\":\"color-no-hex\",\"text\":\"Unexpected hex\"}]}]",
        root,
    )?;
    assert_eq!(sl.len(), 1);
    assert_eq!(sl[0].line, 4);
    assert_eq!(sl[0].column, 3);
    assert_eq!(sl[0].native_code, "color-no-hex");

    assert_eq!(parse_rumdl_or_markdownlint_json("", root)?.len(), 0);
    assert!(parse_rumdl_or_markdownlint_json("not-json", root).is_err());
    let md_both = parse_rumdl_or_markdownlint_json(
        "[{\"file\":\"docs/a.md\",\"line\":9,\"column\":2,\"rule\":\"MD033\",\"message\":\"Inline HTML\"},\
          {\"fileName\":\"docs/b.md\",\"lineNumber\":15,\"errorRange\":[7,3],\"ruleNames\":[\"MD001\",\"heading-increment\"],\"ruleDescription\":\"Heading levels\",\"errorDetail\":\"Expected h2\"}]",
        root,
    )?;
    assert_eq!(md_both.len(), 2);
    assert_eq!(md_both[0].tool, ExternalToolKind::Rumdl);
    assert_eq!(md_both[0].line, 9);
    assert_eq!(md_both[0].column, 2);
    assert_eq!(md_both[1].tool, ExternalToolKind::Markdownlint);
    assert_eq!(md_both[1].line, 15);
    assert_eq!(md_both[1].column, 7);
    assert_eq!(md_both[1].native_code, "MD001");

    let tp = parse_taplo_output(
        "error[invalid_toml]: expected `=`\n  ┌─ C:\\repo\\Cargo.toml:12:5\nwarning: trailing comma\n  --> /repo/mise.toml:3:9\n",
        Path::new("C:/repo"),
    )?;
    assert_eq!(tp.len(), 2);
    assert_eq!(tp[0].path.as_str(), "Cargo.toml");
    assert_eq!(tp[0].line, 12);
    assert_eq!(tp[0].column, 5);
    assert_eq!(tp[0].native_code, "invalid_toml");

    let xl = parse_xmllint_output(
        "C:\\repo\\pom.xml:14: parser error : Opening and ending tag mismatch\nignored noise line\n",
        Path::new("C:/repo"),
    )?;
    assert_eq!(xl.len(), 1);
    assert_eq!(xl[0].path.as_str(), "pom.xml");
    assert_eq!(xl[0].line, 14);
    Ok(())
}

#[test]
fn css_and_xml_syntax_and_comment_justification_boundaries() -> Res {
    // Unclosed CSS comment, string, brace -> GF000; trailing `\` / `\\\n` / `\é` never hang
    assert_eq!(count_diags("styles/err1.css", "/* unclosed\n")?, 1);
    assert_eq!(count_diags("styles/backslash_eof.css", "\\")?, 0);
    assert_eq!(
        count_diags("styles/backslash_nl.css", "\\\n.a { content: \"\\é\"; }\n")?,
        0
    );
    assert_eq!(
        count_diags("styles/err2.css", ".a { content: \"unclosed\n}\n")?,
        1
    );
    assert_eq!(
        count_diags("styles/err3.css", ".a { color: var(--fg);\n")?,
        1
    );
    assert_eq!(
        count_diags(
            "styles/ok.css",
            "/* stylelint-disable color-no-hex -- 0123456789 */\n"
        )?,
        0
    );

    assert_eq!(count_diags("pom.xml", "<project><unclosed></project>")?, 1);
    let bad_pom = "<project><repositories><repository><url>http://insecure.repo/m2</url></repository></repositories><dependencies><dependency><artifactId>x</artifactId><version>RELEASE</version></dependency></dependencies></project>";
    assert_eq!(count_diags("pom.xml", bad_pom)?, 2);
    Ok(())
}

#[test]
fn toml_inline_tables_dotted_reopen_and_markdown_slug_dedup_boundaries() -> Res {
    assert_eq!(
        count_diags("ok4.toml", "x = { a = 1, b = 2, c = 3, d = 4 }\n")?,
        0
    );
    assert_eq!(
        count_diags("bad5.toml", "x = { a = 1, b = 2, c = 3, d = 4, e = 5 }\n")?,
        1
    );
    assert_eq!(
        count_diags("dotted.toml", "a.x = 1\nb.x = 2\na.y = 3\n")?,
        1
    );
    assert_eq!(count_diags("syn.toml", "broken = [1, 2\n")?, 1);

    let md_dups = "# Top\n\n## Sub\n\n## Sub\n\nSee [second](#sub-1).\n";
    assert_eq!(count_diags("docs/dups.md", md_dups)?, 0);
    assert_eq!(
        count_diags("docs/noalt.md", "# Top\n\n![](../assets/pic.png)\n")?,
        1
    );
    Ok(())
}
