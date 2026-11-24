//! Docker Compose service security & hygiene checks (`COMPOSE-PRIVILEGED-OR-HOST`,
//! `COMPOSE-DOCKER-SOCKET`, `COMPOSE-UNPINNED-IMAGE`, `COMPOSE-UNQUOTED-PORTS`).

use gf_core::{ComposeViolation, LangRuleFinding};

use super::anchors_merge::ResolvedYaml;
use super::compose_resources::check_secret_in_env;
use super::events_tree::{YamlScalar, YamlStyle};
use super::yaml_ast::is_extension_key;

const DANGEROUS_CAPS: &[&str] = &["ALL", "SYS_ADMIN", "SYS_PTRACE", "DAC_OVERRIDE"];

fn check_privilege_and_namespaces(
    svc_name: &str,
    svc: &ResolvedYaml,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(s) = svc.get("privileged").and_then(ResolvedYaml::as_scalar)
        && s.value.eq_ignore_ascii_case("true")
    {
        out.push(LangRuleFinding::new(
            ComposeViolation::PrivilegedOrHostNamespace,
            s.span,
            format!("service `{svc_name}` enables `privileged: true`"),
        ));
    }
    for ns_key in ["network_mode", "pid", "ipc"] {
        if let Some(s) = svc.get(ns_key).and_then(ResolvedYaml::as_scalar)
            && s.value.eq_ignore_ascii_case("host")
        {
            out.push(LangRuleFinding::new(
                ComposeViolation::PrivilegedOrHostNamespace,
                s.span,
                format!("service `{svc_name}` shares host namespace via `{ns_key}: host`"),
            ));
        }
    }
    if let Some(u) = svc.get("user").and_then(ResolvedYaml::as_scalar) {
        let trimmed = u.value.trim();
        let is_root = trimmed.eq_ignore_ascii_case("root")
            || trimmed == "0"
            || trimmed.starts_with("0:")
            || trimmed.to_ascii_lowercase().starts_with("root:");
        if is_root {
            out.push(LangRuleFinding::new(
                ComposeViolation::PrivilegedOrHostNamespace,
                u.span,
                format!(
                    "service `{svc_name}` runs explicitly as root (`user: {}`)",
                    u.value
                ),
            ));
        }
    }
    check_cap_add(svc_name, svc, out);
    check_security_opt(svc_name, svc, out);
}

fn check_cap_add(svc_name: &str, svc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = svc.get("cap_add") else {
        return;
    };
    for s in items.iter().filter_map(ResolvedYaml::as_scalar) {
        let upper = s.value.trim().to_ascii_uppercase();
        let cap = upper.strip_prefix("CAP_").unwrap_or(&upper);
        if DANGEROUS_CAPS.contains(&cap) {
            out.push(LangRuleFinding::new(
                ComposeViolation::PrivilegedOrHostNamespace,
                s.span,
                format!(
                    "service `{svc_name}` adds dangerous capability `{}`",
                    s.value
                ),
            ));
        }
    }
}

fn check_security_opt(svc_name: &str, svc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = svc.get("security_opt") else {
        return;
    };
    for s in items.iter().filter_map(ResolvedYaml::as_scalar) {
        if s.value.to_ascii_lowercase().contains("unconfined") {
            out.push(LangRuleFinding::new(
                ComposeViolation::PrivilegedOrHostNamespace,
                s.span,
                format!(
                    "service `{svc_name}` disables confinement via `{}`",
                    s.value
                ),
            ));
        }
    }
}

fn is_docker_sock_path(raw: &str) -> bool {
    let norm = raw.trim();
    norm.contains("/var/run/docker.sock") || norm.contains("/run/docker.sock")
}

fn check_docker_socket(svc_name: &str, svc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = svc.get("volumes") else {
        return;
    };
    for item in items {
        let hit_span = match item {
            ResolvedYaml::Scalar(s) if is_docker_sock_path(&s.value) => Some(s.span),
            ResolvedYaml::Mapping { .. } => item
                .get("source")
                .and_then(ResolvedYaml::as_scalar)
                .filter(|s| is_docker_sock_path(&s.value))
                .map(|s| s.span),
            _ => None,
        };
        if let Some(sp) = hit_span {
            out.push(LangRuleFinding::new(
                ComposeViolation::DockerSocketMount,
                sp,
                format!("service `{svc_name}` mounts the host Docker daemon socket"),
            ));
        }
    }
}

fn is_pinned_compose_image(raw: &str) -> bool {
    let img = raw.trim();
    if img.is_empty() {
        return false;
    }
    let lower = img.to_ascii_lowercase();
    if lower.contains(":-latest}") || lower.contains("-latest}") {
        return false;
    }
    if let Some((_, digest)) = img.split_once("@sha256:") {
        return !digest.is_empty();
    }
    if img.contains("${") {
        return !lower.ends_with(":latest");
    }
    let last_segment = img.rsplit('/').next().unwrap_or(img);
    let Some((name, tag)) = last_segment.split_once(':') else {
        return false;
    };
    !name.is_empty() && !tag.is_empty() && !tag.eq_ignore_ascii_case("latest")
}

fn check_service_image(
    svc_key: &YamlScalar,
    svc: &ResolvedYaml,
    is_override: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let svc_name = svc_key.value.as_str();
    if let Some(img) = svc.get("image").and_then(ResolvedYaml::as_scalar) {
        if !is_pinned_compose_image(&img.value) {
            out.push(LangRuleFinding::new(
                ComposeViolation::UnpinnedOrMissingImage,
                img.span,
                format!(
                    "service `{svc_name}` uses unpinned or `:latest` image `{}`",
                    img.value
                ),
            ));
        }
    } else if !is_override && svc.get("build").is_none() {
        out.push(LangRuleFinding::new(
            ComposeViolation::UnpinnedOrMissingImage,
            svc_key.span,
            format!("service `{svc_name}` declares neither `image:` nor `build:`"),
        ));
    }
}

fn check_unquoted_ports(svc_name: &str, svc: &ResolvedYaml, out: &mut Vec<LangRuleFinding>) {
    let Some(ResolvedYaml::Sequence { items, .. }) = svc.get("ports") else {
        return;
    };
    for s in items.iter().filter_map(ResolvedYaml::as_scalar) {
        if s.style == YamlStyle::Plain {
            out.push(LangRuleFinding::new(
                ComposeViolation::UnquotedPortMapping,
                s.span,
                format!(
                    "service `{svc_name}` has unquoted short port `{}`; wrap in quotes",
                    s.value
                ),
            ));
        }
    }
}

/// Evaluates all per-service Compose rules (`COMPOSE-*`).
pub fn analyze_compose_services(
    services_entries: &[(YamlScalar, ResolvedYaml)],
    is_override: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    for (svc_key, svc_body) in services_entries {
        if is_extension_key(&svc_key.value) {
            continue;
        }
        check_privilege_and_namespaces(&svc_key.value, svc_body, out);
        check_docker_socket(&svc_key.value, svc_body, out);
        check_service_image(svc_key, svc_body, is_override, out);
        check_unquoted_ports(&svc_key.value, svc_body, out);
        check_secret_in_env(&svc_key.value, svc_body, out);
    }
}
