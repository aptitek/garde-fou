//! Property-based tests (`proptest`) for CSS, XML/SVG, TOML, Markdown/MDX, and Tier 3
//! format tool parsers (`stylelint`, `rumdl`, `taplo`, `xmllint`):
//! 1. Arbitrary UTF-8 fuzzing never panics across all 8 parsers.
//! 2. Prepending `+k` leading comment lines shifts 1-based spans covariantly by `+k`.

use std::path::Path;

use gf_core::{Config, RepoPath};
use gf_parsers::{
    parse_rumdl_or_markdownlint_json, parse_source, parse_stylelint_json, parse_taplo_output,
    parse_xmllint_output,
};
use gf_rules::evaluate_document;
use proptest::prelude::*;
use proptest::test_runner::{Config as PropConfig, TestRunner};

#[test]
fn arbitrary_utf8_never_panics_across_css_xml_svg_toml_markdown_and_tool_parsers() {
    let mut runner = TestRunner::new(PropConfig::with_cases(96));
    let res = runner.run(&".*", |fuzz| {
        for rel in [
            "styles/fuzz.css",
            "assets/fuzz.svg",
            "App.csproj",
            "pom.xml",
            "Cargo.toml",
            "mise.toml",
            "docs/fuzz.md",
            "docs/fuzz.mdx",
            ".stylelintrc.json",
            "rumdl.toml",
            "taplo.toml",
            "svgo.config.mjs",
        ] {
            if let Ok(rp) = RepoPath::new(rel) {
                let parsed = parse_source(&rp, &fuzz);
                prop_assert!(parsed.is_ok());
            }
        }
        let root = Path::new("/repo");
        let style_res = parse_stylelint_json(&fuzz, root);
        let md_res = parse_rumdl_or_markdownlint_json(&fuzz, root);
        let taplo_res = parse_taplo_output(&fuzz, root);
        let xml_res = parse_xmllint_output(&fuzz, root);
        prop_assert!(taplo_res.is_ok());
        prop_assert!(xml_res.is_ok());
        prop_assert!(style_res.is_ok() || style_res.is_err());
        prop_assert!(md_res.is_ok() || md_res.is_err());
        Ok(())
    });
    assert!(res.is_ok(), "{res:?}");
}

struct ShiftCase {
    path: &'static str,
    comment_line: &'static str,
    body: &'static str,
}

const SHIFT_CASES: [ShiftCase; 4] = [
    ShiftCase {
        path: "styles/a.css",
        comment_line: "/* pad */\n",
        body: "#hero {\n  color: red;\n}\n",
    },
    ShiftCase {
        path: "assets/a.svg",
        comment_line: "<!-- pad -->\n",
        body: "<svg xmlns=\"http://www.w3.org/2000/svg\"><script>1</script></svg>\n",
    },
    ShiftCase {
        path: "mise.toml",
        comment_line: "# pad\n",
        body: "[tools]\njq = \"latest\"\n",
    },
    ShiftCase {
        path: "docs/a.md",
        comment_line: "<!-- pad -->\n",
        body: "# Title\n\n<div>raw</div>\n",
    },
];

#[test]
fn prepending_k_comment_lines_shifts_css_xml_toml_and_markdown_spans_by_k() {
    let mut runner = TestRunner::new(PropConfig::with_cases(32));
    let res = runner.run(&(1_u32..=25_u32), |k| {
        let cfg = Config::strict_defaults();
        for ShiftCase {
            path,
            comment_line,
            body,
        } in SHIFT_CASES
        {
            let rp = RepoPath::new(path).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let base_doc =
                parse_source(&rp, body).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let base_diags = evaluate_document(&base_doc, &cfg);
            prop_assert!(!base_diags.is_empty());

            let mut shifted_src = comment_line.repeat(k as usize);
            shifted_src.push_str(body);
            let shifted_doc =
                parse_source(&rp, &shifted_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let shifted_diags = evaluate_document(&shifted_doc, &cfg);
            prop_assert_eq!(base_diags.len(), shifted_diags.len());
            for (b, s) in base_diags.iter().zip(shifted_diags.iter()) {
                prop_assert_eq!(&b.rule_id, &s.rule_id);
                prop_assert_eq!(b.span.start_line() + k, s.span.start_line());
            }
        }
        Ok(())
    });
    assert!(res.is_ok(), "{res:?}");
}
