//! Specification-grade `roxmltree` XML & SVG security and manifest extractor
//! (`GF000`, `GF064`, `GF080`).

mod svg_security;
mod xml_manifests;
mod xxe_dtd;

use gf_core::{
    AssetDocument, AssetHazard, AssetHazardKind, LangRuleFinding, Language, RepoPath, Span,
    UastDocument, XmlSvgViolation,
};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};
use svg_security::analyze_svg_dom;
use xml_manifests::check_xml_package_manifest;
use xxe_dtd::{ParsedXmlOutcome, parse_xml_checked};

/// Specification-grade extractor for `.xml`, `.csproj`, `*.props`, and `pom.xml`.
#[derive(Clone, Copy, Debug, Default)]
pub struct XmlExtractor;

impl LanguageExtractor for XmlExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Xml, total_lines, effective_lines)?;

        let parsed: ParsedXmlOutcome<'_> = parse_xml_checked(source);
        doc.syntax_errors = parsed.syntax_errors;
        for (line, msg) in parsed.dtd_xxe_hits {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                XmlSvgViolation::ForbidDtdXxe,
                Span::new(line, line).unwrap_or(Span::FIRST),
                msg,
            ));
        }
        if let Some(dom) = &parsed.dom {
            check_xml_package_manifest(path, dom, &mut doc.lang_rule_findings);
        }
        Ok(doc)
    }
}

/// Extracts an `.svg` asset via `roxmltree`, enforcing XXE/DTD blocking, post-entity XSS/SMIL
/// sanitization, and `<style>` CSS AST bridging (`GF000` / `GF064`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the document span fails.
pub fn extract_svg_roxml(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), Language::Svg, total_lines, effective_lines)?;

    let parsed = parse_xml_checked(source);
    doc.syntax_errors = parsed.syntax_errors;
    let mut hazards = Vec::new();
    for (line, msg) in parsed.dtd_xxe_hits {
        hazards.push(AssetHazard {
            kind: AssetHazardKind::XmlSvgSpec(XmlSvgViolation::ForbidDtdXxe),
            detail: msg,
            span: Span::new(line, line).unwrap_or(Span::FIRST),
        });
    }
    if let Some(dom) = &parsed.dom {
        analyze_svg_dom(dom, &mut hazards, &mut doc.syntax_errors);
    }

    doc.asset = Some(AssetDocument {
        byte_size: u64::try_from(source.len()).unwrap_or(u64::MAX),
        extension: String::from("svg"),
        hazards,
    });
    Ok(doc)
}
