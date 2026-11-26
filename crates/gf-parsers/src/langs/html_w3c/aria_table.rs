//! WAI-ARIA 1.2 specification tables (roles, `aria-*` value types, required properties,
//! and implicit HTML element role redundancies) (`A11Y-ARIA-INVALID` — `GF066`/`GF080`).

use gf_core::{A11yViolation, LangRuleFinding, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AriaValueKind {
    Boolean,
    Tristate,
    OptionalBool,
    Token(&'static [&'static str]),
    TokenList(&'static [&'static str]),
    Integer,
    Decimal,
    IdRef,
    Text,
}

#[rustfmt::skip]
const VALID_ARIA_ROLES: &[&str] = &[
    "alert", "alertdialog", "application", "article", "banner", "blockquote", "button",
    "caption", "cell", "checkbox", "code", "columnheader", "combobox", "complementary",
    "contentinfo", "definition", "deletion", "dialog", "directory", "document", "emphasis",
    "feed", "figure", "form", "generic", "grid", "gridcell", "group", "heading", "img",
    "insertion", "link", "list", "listbox", "listitem", "log", "main", "marquee", "math",
    "meter", "menu", "menubar", "menuitem", "menuitemcheckbox", "menuitemradio", "navigation",
    "none", "note", "option", "paragraph", "presentation", "progressbar", "radio", "radiogroup",
    "region", "row", "rowgroup", "rowheader", "scrollbar", "search", "searchbox", "separator",
    "slider", "spinbutton", "status", "strong", "subscript", "superscript", "switch", "tab",
    "table", "tablist", "tabpanel", "term", "textbox", "time", "timer", "toolbar", "tooltip",
    "tree", "treegrid", "treeitem",
];

const ARIA_AUTOCOMPLETE: &[&str] = &["inline", "list", "both", "none"];
const ARIA_CURRENT: &[&str] = &["page", "step", "location", "date", "time", "true", "false"];
const ARIA_DROPEFFECT: &[&str] = &["copy", "move", "reference", "execute", "popup", "none"];
const ARIA_HASPOPUP: &[&str] = &["true", "false", "menu", "listbox", "tree", "grid", "dialog"];
const ARIA_INVALID_TOKENS: &[&str] = &["true", "false", "spelling", "grammar"];
const ARIA_LIVE: &[&str] = &["off", "polite", "assertive"];
const ARIA_ORIENTATION: &[&str] = &["horizontal", "vertical", "undefined"];
const ARIA_RELEVANT: &[&str] = &["additions", "removals", "text", "all"];
const ARIA_SORT: &[&str] = &["ascending", "descending", "other", "none"];

const ARIA_ATTRIBUTES: &[(&str, AriaValueKind)] = &[
    ("aria-activedescendant", AriaValueKind::IdRef),
    ("aria-atomic", AriaValueKind::Boolean),
    ("aria-autocomplete", AriaValueKind::Token(ARIA_AUTOCOMPLETE)),
    ("aria-braillelabel", AriaValueKind::Text),
    ("aria-brailleroledescription", AriaValueKind::Text),
    ("aria-busy", AriaValueKind::Boolean),
    ("aria-checked", AriaValueKind::Tristate),
    ("aria-colcount", AriaValueKind::Integer),
    ("aria-colindex", AriaValueKind::Integer),
    ("aria-colindextext", AriaValueKind::Text),
    ("aria-colspan", AriaValueKind::Integer),
    ("aria-controls", AriaValueKind::IdRef),
    ("aria-current", AriaValueKind::Token(ARIA_CURRENT)),
    ("aria-describedby", AriaValueKind::IdRef),
    ("aria-description", AriaValueKind::Text),
    ("aria-details", AriaValueKind::IdRef),
    ("aria-disabled", AriaValueKind::Boolean),
    ("aria-dropeffect", AriaValueKind::TokenList(ARIA_DROPEFFECT)),
    ("aria-errormessage", AriaValueKind::IdRef),
    ("aria-expanded", AriaValueKind::OptionalBool),
    ("aria-flowto", AriaValueKind::IdRef),
    ("aria-grabbed", AriaValueKind::OptionalBool),
    ("aria-haspopup", AriaValueKind::Token(ARIA_HASPOPUP)),
    ("aria-hidden", AriaValueKind::Boolean),
    ("aria-invalid", AriaValueKind::Token(ARIA_INVALID_TOKENS)),
    ("aria-keyshortcuts", AriaValueKind::Text),
    ("aria-label", AriaValueKind::Text),
    ("aria-labelledby", AriaValueKind::IdRef),
    ("aria-level", AriaValueKind::Integer),
    ("aria-live", AriaValueKind::Token(ARIA_LIVE)),
    ("aria-modal", AriaValueKind::Boolean),
    ("aria-multiline", AriaValueKind::Boolean),
    ("aria-multiselectable", AriaValueKind::Boolean),
    ("aria-orientation", AriaValueKind::Token(ARIA_ORIENTATION)),
    ("aria-owns", AriaValueKind::IdRef),
    ("aria-placeholder", AriaValueKind::Text),
    ("aria-posinset", AriaValueKind::Integer),
    ("aria-pressed", AriaValueKind::Tristate),
    ("aria-readonly", AriaValueKind::Boolean),
    ("aria-relevant", AriaValueKind::TokenList(ARIA_RELEVANT)),
    ("aria-required", AriaValueKind::Boolean),
    ("aria-roledescription", AriaValueKind::Text),
    ("aria-rowcount", AriaValueKind::Integer),
    ("aria-rowindex", AriaValueKind::Integer),
    ("aria-rowindextext", AriaValueKind::Text),
    ("aria-rowspan", AriaValueKind::Integer),
    ("aria-selected", AriaValueKind::OptionalBool),
    ("aria-setsize", AriaValueKind::Integer),
    ("aria-sort", AriaValueKind::Token(ARIA_SORT)),
    ("aria-valuemax", AriaValueKind::Decimal),
    ("aria-valuemin", AriaValueKind::Decimal),
    ("aria-valuenow", AriaValueKind::Decimal),
    ("aria-valuetext", AriaValueKind::Text),
];

const REDUNDANT_ELEMENT_ROLES: &[(&str, &str)] = &[
    ("button", "button"),
    ("nav", "navigation"),
    ("main", "main"),
    ("aside", "complementary"),
    ("article", "article"),
    ("ul", "list"),
    ("ol", "list"),
    ("li", "listitem"),
    ("table", "table"),
    ("form", "form"),
];

const REQUIRED_ROLE_ATTRS: &[(&str, &[&str])] = &[
    ("checkbox", &["aria-checked"]),
    ("switch", &["aria-checked"]),
    ("menuitemcheckbox", &["aria-checked"]),
    ("menuitemradio", &["aria-checked"]),
    ("radio", &["aria-checked"]),
    ("slider", &["aria-valuenow"]),
    ("spinbutton", &["aria-valuenow"]),
    ("scrollbar", &["aria-controls", "aria-valuenow"]),
    ("heading", &["aria-level"]),
];

fn is_valid_aria_scalar(kind: AriaValueKind, raw: &str) -> bool {
    let val = raw.trim();
    if val.is_empty() {
        return false;
    }
    let low = val.to_ascii_lowercase();
    match kind {
        AriaValueKind::Boolean => matches!(low.as_str(), "true" | "false"),
        AriaValueKind::Tristate => matches!(low.as_str(), "true" | "false" | "mixed" | "undefined"),
        AriaValueKind::OptionalBool => matches!(low.as_str(), "true" | "false" | "undefined"),
        AriaValueKind::Token(allowed) => allowed.contains(&low.as_str()),
        AriaValueKind::TokenList(allowed) => {
            low.split_whitespace().all(|tok| allowed.contains(&tok))
        }
        AriaValueKind::Integer => val.parse::<i64>().is_ok(),
        AriaValueKind::Decimal => val.parse::<f64>().is_ok_and(f64::is_finite),
        AriaValueKind::IdRef | AriaValueKind::Text => true,
    }
}

/// Attribute representation `(name, static_literal_value, is_dynamic_expr)`.
#[derive(Clone, Debug)]
pub(in crate::langs) struct HtmlAttrView<'a> {
    pub name: &'a str,
    pub static_val: Option<&'a str>,
    pub is_dynamic: bool,
}

/// Normalized HTML/JSX/Astro element node for W3C + WCAG tree validation.
#[derive(Clone, Debug)]
pub(in crate::langs) struct HtmlElementNode<'a> {
    pub tag: &'a str,
    pub attrs: Vec<HtmlAttrView<'a>>,
    pub span: Span,
    pub has_any_child_content: bool,
    pub children: Vec<Self>,
}

fn required_attrs_for_role(role: &str) -> &'static [&'static str] {
    REQUIRED_ROLE_ATTRS
        .iter()
        .find_map(|&(r, reqs)| (r == role).then_some(reqs))
        .unwrap_or(&[])
}

fn check_role_attr(
    tag: &str,
    attrs: &[HtmlAttrView<'_>],
    span: Span,
    out: &mut Vec<LangRuleFinding>,
) {
    let Some(role_val) = attrs
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("role"))
        .and_then(|a| a.static_val)
    else {
        return;
    };
    let role = role_val.trim();
    if !VALID_ARIA_ROLES.contains(&role) {
        out.push(LangRuleFinding::new(
            A11yViolation::AriaInvalid,
            span,
            format!("invalid WAI-ARIA 1.2 `role=\"{role}\"` on `<{tag}>`"),
        ));
        return;
    }
    if REDUNDANT_ELEMENT_ROLES.contains(&(tag, role)) {
        out.push(LangRuleFinding::new(
            A11yViolation::AriaInvalid,
            span,
            format!("redundant `role=\"{role}\"` on native `<{tag}>` element"),
        ));
    }
    for &req in required_attrs_for_role(role) {
        if !attrs.iter().any(|a| a.name.eq_ignore_ascii_case(req)) {
            out.push(LangRuleFinding::new(
                A11yViolation::AriaInvalid,
                span,
                format!("`role=\"{role}\"` on `<{tag}>` is missing required `{req}` state"),
            ));
        }
    }
}

/// Validates `role` and `aria-*` attributes on an intrinsic HTML/JSX/Astro element.
pub(super) fn validate_element_aria(
    tag: &str,
    attrs: &[HtmlAttrView<'_>],
    span: Span,
    out: &mut Vec<LangRuleFinding>,
) {
    check_role_attr(tag, attrs, span, out);
    for attr in attrs {
        let low_name = attr.name.to_ascii_lowercase();
        if !low_name.starts_with("aria-") {
            continue;
        }
        let Some(&(_, kind)) = ARIA_ATTRIBUTES.iter().find(|&&(k, _)| k == low_name) else {
            out.push(LangRuleFinding::new(
                A11yViolation::AriaInvalid,
                span,
                format!(
                    "unknown or misspelled ARIA attribute `{}` on `<{tag}>`",
                    attr.name
                ),
            ));
            continue;
        };
        if !attr.is_dynamic
            && !attr
                .static_val
                .is_some_and(|v| is_valid_aria_scalar(kind, v))
        {
            out.push(LangRuleFinding::new(
                A11yViolation::AriaInvalid,
                span,
                format!(
                    "invalid value `{}` for `{}` on `<{tag}>`",
                    attr.static_val.unwrap_or(""),
                    attr.name
                ),
            ));
        }
    }
}
