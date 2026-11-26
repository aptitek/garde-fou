//! Phase 4.3: Property-based invariants (`proptest`) for Astro 3-zone splitting,
//! W3C HTML5 & WAI-ARIA 1.2 / WCAG 2.2 AA scanning, `wrangler.*` / `panda.config.*`,
//! Panda CSS + M3E, Cloudflare Workers, and Drizzle ORM (`GF061`, `GF080`, `GF081`).

use std::path::Path;

use gf_core::{DomainLayoutConfig, Language, RepoPath, SqlDialectKind};
use gf_parsers::{
    detect_language, parse_astro_check_output, parse_html_validate_json, parse_source,
    parse_source_with_context,
};
use proptest::prelude::*;

#[test]
fn astro_and_html_extensions_resolve_to_dedicated_languages()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        detect_language(&RepoPath::new("src/pages/index.astro")?),
        Language::Astro
    );
    assert_eq!(
        detect_language(&RepoPath::new("public/index.html")?),
        Language::Html
    );
    assert_eq!(
        detect_language(&RepoPath::new("public/legacy.htm")?),
        Language::Html
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn arbitrary_utf8_never_panics_across_astro_html_wrangler_and_ts_analyzers(
        raw in "[\\s\\S]{0,512}",
        frontmatter in "[\\s\\S]{0,128}",
        style_body in "[\\s\\S]{0,128}",
    ) {
        let domains = DomainLayoutConfig::strict_defaults();
        for (rel, hint) in [
            ("src/pages/fuzz.astro", None),
            ("public/fuzz.html", None),
            ("wrangler.toml", None),
            ("wrangler.jsonc", None),
            ("panda.config.ts", None),
            ("src/routes/fuzz.tsx", Some(SqlDialectKind::CloudflareD1)),
        ] {
            if let Ok(rp) = RepoPath::new(rel) {
                let _ = parse_source_with_context(&rp, &raw, &domains, hint);
            }
        }
        let composite_astro = format!("---\n{frontmatter}\n---\n<div>{raw}</div>\n<style>\n{style_body}\n</style>\n");
        if let Ok(rp) = RepoPath::new("src/pages/composite.astro") {
            let _ = parse_source(&rp, &composite_astro);
        }
        let _ = parse_astro_check_output(&raw, Path::new("/repo"));
        let _ = parse_html_validate_json(&raw, Path::new("/repo"));
    }

    #[test]
    fn astro_and_html_line_numbers_shift_covariantly_with_leading_blank_lines(
        k in 1_usize..20,
    ) {
        let base_html = "<main>\n  <a href=\"/x\"><button type=\"button\">Go</button></a>\n</main>\n";
        let shifted_html = format!("{}{base_html}", "\n".repeat(k));
        let path_h = RepoPath::new("public/cov.html").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let h_base = parse_source(&path_h, base_html).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let h_shift = parse_source(&path_h, &shifted_html).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let delta = u32::try_from(k).unwrap_or(0);
        prop_assert_eq!(h_base.lang_rule_findings.len(), h_shift.lang_rule_findings.len());
        for (b, s) in h_base.lang_rule_findings.iter().zip(h_shift.lang_rule_findings.iter()) {
            prop_assert_eq!(b.code(), s.code());
            prop_assert_eq!(b.span.start_line() + delta, s.span.start_line());
        }
    }

    #[test]
    fn astro_frontmatter_padding_shifts_template_and_style_lines_covariantly(
        k in 1_usize..20,
    ) {
        let fm_pad = "\n".repeat(k);
        let astro_base = "---\nconst x = 1;\n---\n<main>\n  <div set:html={x} />\n</main>\n<style>\n  .x { color: #abcdef; }\n</style>\n";
        let astro_padded = format!("---\nconst x = 1;{fm_pad}\n---\n<main>\n  <div set:html={{x}} />\n</main>\n<style>\n  .x {{ color: #abcdef; }}\n</style>\n");
        let path = RepoPath::new("src/pages/pad.astro").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let d_base = parse_source(&path, astro_base).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let d_pad = parse_source(&path, &astro_padded).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let delta = u32::try_from(k).unwrap_or(0);

        prop_assert_eq!(d_base.lang_rule_findings.len(), d_pad.lang_rule_findings.len());
        for (b, p) in d_base.lang_rule_findings.iter().zip(d_pad.lang_rule_findings.iter()) {
            prop_assert_eq!(b.code(), p.code());
            prop_assert_eq!(b.span.start_line() + delta, p.span.start_line());
        }
        let f_base = d_base.forme.as_ref().map(|f| &f.findings[0]);
        let f_pad = d_pad.forme.as_ref().map(|f| &f.findings[0]);
        if let (Some(fb), Some(fp)) = (f_base, f_pad) {
            prop_assert_eq!(fb.span.start_line() + delta, fp.span.start_line());
        }
    }
}
