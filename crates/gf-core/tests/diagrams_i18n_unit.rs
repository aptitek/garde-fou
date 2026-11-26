//! Unit tests for Mermaid, `PlantUML`, `i18n` v2 violation types and embedded FR/EN `ui_i18n`.

use gf_core::{
    DiagramI18nViolation, I18nViolation, MermaidViolation, PlantUmlViolation, UiLocale,
    UiLocalePolicy, ui_tr, ui_tr_fmt,
};

#[rustfmt::skip]
const CASES: &[(DiagramI18nViolation, &str, &str)] = &[
    (DiagramI18nViolation::Mermaid(MermaidViolation::UnknownOrEmptyDiagram), "MERMAID-UNKNOWN-OR-EMPTY-DIAGRAM", "flowchart"),
    (DiagramI18nViolation::Mermaid(MermaidViolation::UnclosedBlockOrShape), "MERMAID-UNCLOSED-BLOCK-OR-SHAPE", "subgraph"),
    (DiagramI18nViolation::Mermaid(MermaidViolation::ReservedEndNode), "MERMAID-RESERVED-END-NODE", "END_NODE"),
    (DiagramI18nViolation::Mermaid(MermaidViolation::SpaghettiComplexity), "MERMAID-SPAGHETTI-COMPLEXITY", "max_nodes"),
    (DiagramI18nViolation::Mermaid(MermaidViolation::UnsafeSecurityOrXss), "MERMAID-UNSAFE-SECURITY-OR-XSS", "securityLevel"),
    (DiagramI18nViolation::PlantUml(PlantUmlViolation::UnbalancedStructure), "PLANTUML-UNBALANCED-STRUCTURE", "@end"),
    (DiagramI18nViolation::PlantUml(PlantUmlViolation::SsrfOrLocalLeak), "PLANTUML-SSRF-OR-LOCAL-LEAK", "%getenv"),
    (DiagramI18nViolation::PlantUml(PlantUmlViolation::BrokenInclude), "PLANTUML-BROKEN-INCLUDE", "!include"),
    (DiagramI18nViolation::PlantUml(PlantUmlViolation::XssLink), "PLANTUML-XSS-LINK", "javascript:"),
    (DiagramI18nViolation::I18n(I18nViolation::InvalidIcuSyntax), "I18N-INVALID-ICU-SYNTAX", "other"),
    (DiagramI18nViolation::I18n(I18nViolation::PlaceholderMismatch), "I18N-PLACEHOLDER-MISMATCH", "placeholder"),
    (DiagramI18nViolation::I18n(I18nViolation::EmptyOrFuzzyOrXss), "I18N-EMPTY-OR-FUZZY-OR-XSS", "fuzzy"),
];

#[test]
fn diagram_and_i18n_violations_expose_exact_codes_display_and_remediations() {
    for &(v, code, needle) in CASES {
        assert_eq!(v.code(), code);
        assert_eq!(format!("{v}"), code);
        assert!(v.remediation().contains(needle));
    }
}

#[test]
fn ui_locale_policy_and_embedded_fr_en_catalog_resolve_deterministically() {
    assert_eq!(UiLocale::En.as_str(), "en");
    assert_eq!(UiLocale::Fr.as_str(), "fr");
    assert_eq!(
        UiLocalePolicy::from_config_str("FR"),
        Some(UiLocalePolicy::Fr)
    );
    assert_eq!(
        UiLocalePolicy::from_config_str("en_US"),
        Some(UiLocalePolicy::En)
    );
    assert_eq!(
        UiLocalePolicy::from_config_str("auto"),
        Some(UiLocalePolicy::Auto)
    );
    assert_eq!(UiLocalePolicy::from_config_str("xx"), None);
    assert_eq!(UiLocalePolicy::Auto.as_str(), "auto");
    assert_eq!(UiLocalePolicy::Fr.as_str(), "fr");
    assert_eq!(UiLocalePolicy::En.as_str(), "en");
    assert_eq!(
        UiLocalePolicy::Auto.resolve(Some("fr_FR.UTF-8")),
        UiLocale::Fr
    );
    assert_eq!(
        UiLocalePolicy::Auto.resolve(Some("en_US.UTF-8")),
        UiLocale::En
    );
    assert_eq!(UiLocalePolicy::Auto.resolve(None), UiLocale::En);
    assert_eq!(UiLocalePolicy::Fr.resolve(None), UiLocale::Fr);
    assert_eq!(UiLocalePolicy::En.resolve(Some("fr_FR")), UiLocale::En);

    assert!(ui_tr(UiLocale::Fr, "cli.lock_ok").contains("garde.lock"));
    assert!(ui_tr(UiLocale::En, "cli.lock_ok").contains("verified"));
    assert_eq!(ui_tr(UiLocale::Fr, "missing.key"), "missing.key");
    let msg = ui_tr_fmt(
        UiLocale::En,
        "report.cache_summary",
        &[("hits", "5"), ("misses", "1")],
    );
    assert_eq!(msg, "Cache: 5 hits, 1 misses");
}
