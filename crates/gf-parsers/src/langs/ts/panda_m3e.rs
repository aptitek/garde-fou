//! Panda CSS build-time token discipline (`PANDA-*`) and Material Design 3 Expressive (`M3E-*`).

use super::style_and_i18n::{contains_raw_color, is_standalone_css_unit, unquote_literal};
use crate::cst::{collect_syntax_errors, node_span, node_text, parse_with_language};
use gf_core::{
    LangRuleFinding, M3eViolation, PandaViolation, Span, ToolConfigFinding, ToolConfigViolation,
    UastDocument, WebEdgeConfigIssue,
};
use tree_sitter::Node;

#[rustfmt::skip]
const PRIMITIVE_PALETTES: &[&str] = &[
    "amber", "blue", "cyan", "emerald", "fuchsia", "gray", "green", "indigo", "lime", "neutral",
    "orange", "pink", "purple", "red", "rose", "sky", "slate", "solarized", "stone", "teal",
    "violet", "yellow", "zinc",
];
#[rustfmt::skip]
const NON_GEOMETRIC_PROPS: &[&str] = &[
    "background", "background-color", "backgroundColor", "bg", "border-color",
    "borderColor", "box-shadow", "boxShadow", "color", "fill", "opacity", "stroke",
];
#[rustfmt::skip]
const MD3_SHAPES: &[&str] = &[
    "none", "xs", "extraSmall", "sm", "small", "md", "medium",
    "lg", "large", "xl", "extraLarge", "2xl", "extraExtraLarge", "full",
];
#[rustfmt::skip]
const SURFACE_INKS: &[&str] = &[
    "onSurface", "onSurfaceVariant", "primary", "secondary", "tertiary", "error",
];
#[rustfmt::skip]
const MD3_TONAL_PAIRS: &[(&str, &[&str])] = &[
    ("primary", &["onPrimary"]), ("onPrimary", &["primary"]),
    ("primaryContainer", &["onPrimaryContainer"]), ("secondary", &["onSecondary"]),
    ("secondaryContainer", &["onSecondaryContainer"]), ("tertiary", &["onTertiary"]),
    ("tertiaryContainer", &["onTertiaryContainer"]), ("error", &["onError"]),
    ("errorContainer", &["onErrorContainer"]),
    ("inverseSurface", &["inverseOnSurface", "inversePrimary"]),
];

#[rustfmt::skip]
fn is_valid_m3e_elevation(t: &str) -> bool {
    t == "none"
        || t.contains("elevation.level")
        || t.contains("--elevation-level")
        || matches!(t, "level0" | "level1" | "level2" | "level3" | "level4" | "level5")
}

#[rustfmt::skip]
fn check_m3e_elevation_shape_typo(key: &str, val: &str, span: Span, out: &mut Vec<LangRuleFinding>) {
    let t = val.trim();
    let bad = match key {
        "boxShadow" | "shadow" => !is_valid_m3e_elevation(t),
        "borderRadius" | "rounded" => !is_valid_m3e_shape(t),
        "textStyle" => !is_valid_m3e_text_style(t),
        _ => false,
    };
    if bad {
        out.push(LangRuleFinding::new(
            M3eViolation::RawElevationShapeTypo,
            span,
            format!("`{key}: '{t}'` is outside MD3 elevation/shape/typography token scales"),
        ));
    }
}

#[rustfmt::skip]
fn check_tonal_pair(bg_role: Option<(&str, Span)>, fg_role: Option<(&str, Span)>, out: &mut Vec<LangRuleFinding>) {
    if let (Some((bg, _)), Some((fg, fg_span))) = (bg_role, fg_role)
        && let Some(allowed) = expected_md3_inks(bg)
        && !allowed.contains(&fg.rsplit('.').next().unwrap_or(fg))
    {
        out.push(LangRuleFinding::new(
            M3eViolation::TonalPairing,
            fg_span,
            format!("MD3 tonal mismatch: surface `{bg}` requires `{allowed:?}`, found `{fg}`"),
        ));
    }
}

#[rustfmt::skip]
fn inspect_nested_panda_object(key: &str, v_node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if key == "_hover" && node_text(v_node, source).contains("opacity") {
        out.push(LangRuleFinding::new(
            M3eViolation::TouchTargetAndStateLayer,
            node_span(v_node),
            String::from("`_hover: { opacity }` replaces MD3 state-layer tokens with arbitrary opacity"),
        ));
    }
    inspect_panda_style_object(v_node, source, out);
}

#[rustfmt::skip]
fn is_primitive_palette_token(val: &str) -> bool {
    let clean = val.trim().trim_start_matches("colors.");
    let Some((palette, shade)) = clean.split_once('.') else { return false };
    let known = PRIMITIVE_PALETTES.iter().any(|p| palette.eq_ignore_ascii_case(p));
    let numeric = !shade.is_empty() && shade.bytes().all(|b| b.is_ascii_digit());
    known && (numeric || shade.starts_with("base0") || shade.starts_with("base1"))
}

#[rustfmt::skip]
fn is_arbitrary_or_primitive_value(val: &str) -> bool {
    let t = val.trim();
    if (t.starts_with('[') && t.ends_with(']')) || t.ends_with('!') || t.ends_with("!important") || is_primitive_palette_token(t) {
        return true;
    }
    !t.contains("var(--") && !t.contains("token(") && (contains_raw_color(t) || is_standalone_css_unit(t))
}

fn expected_md3_inks(bg: &str) -> Option<&'static [&'static str]> {
    let role = bg.rsplit('.').next().unwrap_or(bg);
    if role.starts_with("surface") {
        return Some(SURFACE_INKS);
    }
    MD3_TONAL_PAIRS
        .iter()
        .find_map(|&(surf, inks)| (surf == role).then_some(inks))
}

#[rustfmt::skip]
fn has_spatial_spring_on_non_geometry(transition_val: &str) -> bool {
    if transition_val.to_ascii_lowercase().contains("cubic-bezier(") {
        return true;
    }
    for clause in transition_val.split(',') {
        let trimmed = clause.trim();
        let is_spatial = trimmed.contains("Spatial") || trimmed.contains("-spatial") || trimmed.contains("spatial.");
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        if is_spatial && (NON_GEOMETRIC_PROPS.contains(&first_word) || first_word == "all") {
            return true;
        }
    }
    false
}

fn is_valid_m3e_shape(t: &str) -> bool {
    let tail = t.rsplit('.').next().unwrap_or(t);
    MD3_SHAPES.contains(&tail) || t.starts_with("var(--shape-") || t.starts_with("var(--radius-")
}

fn is_valid_m3e_text_style(t: &str) -> bool {
    let low = t.to_ascii_lowercase();
    ["display", "headline", "title", "body", "label"]
        .iter()
        .any(|p| low.starts_with(p) || low.contains(&format!(".{p}")))
}

fn is_sub_48dp_target(key: &str, val: &str) -> bool {
    if !matches!(key, "minW" | "minH" | "minWidth" | "minHeight") {
        return false;
    }
    let low = val.trim().to_ascii_lowercase();
    if let Some(px) = low.strip_suffix("px").or_else(|| low.strip_suffix("dp")) {
        return px.trim().parse::<f32>().is_ok_and(|v| v > 0.0 && v < 48.0);
    }
    low.strip_suffix("rem")
        .is_some_and(|r| r.trim().parse::<f32>().is_ok_and(|v| v > 0.0 && v < 3.0))
}

fn inspect_panda_scalar_pair(key: &str, val_str: &str, span: Span, out: &mut Vec<LangRuleFinding>) {
    if is_arbitrary_or_primitive_value(val_str) {
        out.push(LangRuleFinding::new(
            PandaViolation::ArbitraryOrPrimitive,
            span,
            format!("`{key}: '{val_str}'` uses an arbitrary or primitive palette value instead of a semantic token"),
        ));
    }
    if is_sub_48dp_target(key, val_str) {
        out.push(LangRuleFinding::new(
            M3eViolation::TouchTargetAndStateLayer,
            span,
            format!("`{key}: '{val_str}'` violates the 48x48dp (`3rem`) MD3 minimum interactive touch target"),
        ));
    }
    if matches!(key, "transition" | "animation") && has_spatial_spring_on_non_geometry(val_str) {
        out.push(LangRuleFinding::new(
            M3eViolation::SpringPhysicsMismatch,
            span,
            format!("`{key}: '{val_str}'` applies a `Spatial` overshoot spring or raw cubic-bezier to a non-geometric property"),
        ));
    }
    check_m3e_elevation_shape_typo(key, val_str, span, out);
}

fn inspect_panda_style_object(obj: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let (mut bg_role, mut fg_role) = (None, None);
    let mut c = obj.walk();
    for ch in obj.named_children(&mut c) {
        let (Some(k_node), Some(v_node)) = (
            ch.child_by_field_name("key"),
            ch.child_by_field_name("value"),
        ) else {
            continue;
        };
        let key = node_text(k_node, source).trim().trim_matches(['"', '\'']);
        let span = node_span(ch);
        if v_node.kind() == "object" {
            inspect_nested_panda_object(key, v_node, source, out);
        } else if let Some(val_str) = unquote_literal(node_text(v_node, source)) {
            if matches!(key, "bg" | "background" | "backgroundColor") {
                bg_role = Some((val_str.trim(), span));
            } else if matches!(key, "color" | "fg" | "fill") {
                fg_role = Some((val_str.trim(), span));
            }
            inspect_panda_scalar_pair(key, val_str, span, out);
        }
    }
    check_tonal_pair(bg_role, fg_role, out);
}

/// Walks a TS/TSX CST for Panda CSS (`css`, `cva`, `sva`) and M3E rules (`PANDA-*`, `M3E-*`).
pub fn extract_panda_and_m3e(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    if root.kind() == "call_expression"
        && let Some(func) = root.child_by_field_name("function")
        && matches!(node_text(func, source).trim(), "css" | "cva" | "sva")
        && let Some(args) = root.child_by_field_name("arguments")
    {
        let mut c = args.walk();
        for arg in args.named_children(&mut c) {
            if arg.kind() == "object" {
                inspect_panda_style_object(arg, source, out);
            }
        }
    }
    let mut cursor = root.walk();
    for ch in root.children(&mut cursor) {
        extract_panda_and_m3e(ch, source, out);
    }
}

/// Audits `panda.config.{ts,mjs,js}` for `strictTokens: true` (`PANDA-CONFIG-STRICT` — `GF081`).
#[rustfmt::skip]
pub fn audit_panda_config(source: &str, doc: &mut UastDocument) {
    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    if let Ok(tree) = parse_with_language(&ts_lang, source) {
        doc.syntax_errors = collect_syntax_errors(tree.root_node());
    }
    let compact = source.replace([' ', '\t'], "");
    if !compact.contains("strictTokens:true") || compact.contains("strictPropertyValues:false") {
        doc.tool_config_findings.push(ToolConfigFinding {
            violation: ToolConfigViolation::WebEdge(WebEdgeConfigIssue::PandaStrictTokensMissingOrFalse),
            span: Span::FIRST,
            message: String::from("[PANDA-CONFIG-STRICT] `panda.config.*` must enable `strictTokens: true` without disabling `strictPropertyValues`"),
        });
    }
}
