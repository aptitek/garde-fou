//! Docker Compose `environment:` secret checks (`COMPOSE-SECRET-IN-ENV`) and top-level
//! `volumes` / `networks` / `secrets` / `configs` coherence (`COMPOSE-ORPHAN-OR-UNDECLARED-RESOURCE`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{ComposeViolation, LangRuleFinding, Span};

use super::anchors_merge::ResolvedYaml;
use super::events_tree::YamlScalar;
use super::yaml_ast::is_extension_key;

const BENIGN_KEY_SUFFIXES: &[&str] = &[
    "PUBLIC_KEY",
    "HOST_KEY",
    "PRIMARY_KEY",
    "FOREIGN_KEY",
    "SORT_KEY",
    "PARTITION_KEY",
    "CACHE_KEY",
    "ROUTING_KEY",
    "IDEMPOTENCY_KEY",
    "TRANSLATION_KEY",
];

const SECRET_KEY_INFIXES: &[&str] = &["PASSWORD", "PASSWD", "SECRET", "APIKEY"];

fn is_secret_env_key(key: &str) -> bool {
    let up = key.trim().to_ascii_uppercase();
    !BENIGN_KEY_SUFFIXES.iter().any(|suf| up.ends_with(suf))
        && (SECRET_KEY_INFIXES.iter().any(|inf| up.contains(inf))
            || up.ends_with("_TOKEN")
            || up.ends_with("_KEY"))
}

fn is_safe_env_value(val: &str) -> bool {
    let t = val.trim();
    t.is_empty() || t == "null" || t == "~" || t.contains("${")
}

fn emit_env_secret(svc_name: &str, key_name: &str, span: Span, out: &mut Vec<LangRuleFinding>) {
    out.push(LangRuleFinding::new(
        ComposeViolation::PlaintextSecretInEnv,
        span,
        format!("service `{svc_name}` hardcodes secret `{key_name}` in `environment:`"),
    ));
}

fn hardcoded_secret_key<'a>(k: &'a str, v: &str) -> Option<&'a str> {
    (is_secret_env_key(k) && !is_safe_env_value(v)).then_some(k.trim())
}

fn check_secret_env_map(
    svc_name: &str,
    entries: &[(YamlScalar, ResolvedYaml)],
    out: &mut Vec<LangRuleFinding>,
) {
    for (k, v) in entries {
        if let Some(vs) = v.as_scalar()
            && let Some(bad_k) = hardcoded_secret_key(&k.value, &vs.value)
        {
            emit_env_secret(svc_name, bad_k, k.span, out);
        }
    }
}

fn check_secret_env_seq(svc_name: &str, items: &[ResolvedYaml], out: &mut Vec<LangRuleFinding>) {
    for s in items.iter().filter_map(ResolvedYaml::as_scalar) {
        if let Some((k, v)) = s.value.split_once('=')
            && let Some(bad_k) = hardcoded_secret_key(k, v)
        {
            emit_env_secret(svc_name, bad_k, s.span, out);
        }
    }
}

/// Checks `environment:` (both mapping and sequence forms) for plaintext secrets (`COMPOSE-SECRET-IN-ENV`).
pub fn check_secret_in_env(svc_name: &str, svc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    match svc.get("environment") {
        Some(ResolvedYaml::Mapping { entries, .. }) => check_secret_env_map(svc_name, entries, out),
        Some(ResolvedYaml::Sequence { items, .. }) => check_secret_env_seq(svc_name, items, out),
        _ => {}
    }
}

const fn is_windows_drive_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

fn is_bind_or_interpolated_source(src: &str) -> bool {
    src.is_empty()
        || src.starts_with('.')
        || src.starts_with('/')
        || src.starts_with('~')
        || src.starts_with('$')
        || is_windows_drive_path(src)
}

fn collect_service_volume_refs(svc: &ResolvedYaml, refs: &mut Vec<(String, Span)>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = svc.get("volumes") else {
        return;
    };
    for item in items {
        match item {
            ResolvedYaml::Scalar(s) => {
                let raw = s.value.trim();
                if !is_windows_drive_path(raw)
                    && let Some((src, _)) = raw.split_once(':')
                    && !is_bind_or_interpolated_source(src.trim())
                {
                    refs.push((String::from(src.trim()), s.span));
                }
            }
            ResolvedYaml::Mapping { .. } => {
                let vol_type = item
                    .get("type")
                    .and_then(ResolvedYaml::as_scalar)
                    .map_or("volume", |t| t.value.as_str());
                if vol_type == "volume"
                    && let Some(src) = item.get("source").and_then(ResolvedYaml::as_scalar)
                    && !is_bind_or_interpolated_source(src.value.trim())
                {
                    refs.push((String::from(src.value.trim()), src.span));
                }
            }
            ResolvedYaml::Sequence { .. } => {}
        }
    }
}

fn item_named_scalar(item: &ResolvedYaml) -> Option<&YamlScalar> {
    item.as_scalar()
        .or_else(|| item.get("source").and_then(ResolvedYaml::as_scalar))
}

fn collect_named_refs(svc: &ResolvedYaml, key: &str, refs: &mut Vec<(String, Span)>) {
    match svc.get(key) {
        Some(ResolvedYaml::Sequence { items, .. }) => {
            refs.extend(
                items
                    .iter()
                    .filter_map(item_named_scalar)
                    .map(|s| (s.value.clone(), s.span)),
            );
        }
        Some(ResolvedYaml::Mapping { entries, .. }) => {
            refs.extend(
                entries
                    .iter()
                    .filter(|(k, _)| !is_extension_key(&k.value))
                    .map(|(k, _)| (k.value.clone(), k.span)),
            );
        }
        _ => {}
    }
}

fn declared_resource_map(root: &ResolvedYaml, key: &str) -> BTreeMap<String, Span> {
    let Some(ResolvedYaml::Mapping { entries, .. }) = root.get(key) else {
        return BTreeMap::new();
    };
    entries
        .iter()
        .filter(|(k, _)| !is_extension_key(&k.value))
        .map(|(k, _)| (k.value.clone(), k.span))
        .collect()
}

fn is_implicit_default_net(kind_label: &str, name: &str) -> bool {
    kind_label == "network" && name == "default"
}

fn verify_resource_kind(
    kind_label: &str,
    declared: &BTreeMap<String, Span>,
    refs: &[(String, Span)],
    out: &mut Vec<LangRuleFinding>,
) {
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for (name, sp) in refs {
        if !is_implicit_default_net(kind_label, name) {
            used.insert(name.as_str());
            if !declared.contains_key(name) {
                out.push(LangRuleFinding::new(
                    ComposeViolation::OrphanOrUndeclaredResource,
                    *sp,
                    format!("service references undeclared top-level {kind_label} `{name}`"),
                ));
            }
        }
    }
    for (decl_name, decl_span) in declared {
        if !is_implicit_default_net(kind_label, decl_name) && !used.contains(decl_name.as_str()) {
            out.push(LangRuleFinding::new(
                ComposeViolation::OrphanOrUndeclaredResource,
                *decl_span,
                format!(
                    "top-level {kind_label} `{decl_name}` is declared but never referenced by any service"
                ),
            ));
        }
    }
}

/// Checks undeclared or unused top-level `volumes`, `networks`, `secrets`, and `configs`.
pub fn check_compose_resources(
    root: &ResolvedYaml,
    services: &[(YamlScalar, ResolvedYaml)],
    out: &mut Vec<LangRuleFinding>,
) {
    let mut vol_refs = Vec::new();
    for (k, svc) in services {
        if !is_extension_key(&k.value) {
            collect_service_volume_refs(svc, &mut vol_refs);
        }
    }
    verify_resource_kind(
        "volume",
        &declared_resource_map(root, "volumes"),
        &vol_refs,
        out,
    );
    for (yaml_key, label) in [
        ("networks", "network"),
        ("secrets", "secret"),
        ("configs", "config"),
    ] {
        let mut refs = Vec::new();
        for (k, svc) in services {
            if !is_extension_key(&k.value) {
                collect_named_refs(svc, yaml_key, &mut refs);
            }
        }
        verify_resource_kind(label, &declared_resource_map(root, yaml_key), &refs, out);
    }
}
