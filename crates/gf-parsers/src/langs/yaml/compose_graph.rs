//! Docker Compose `depends_on` graph validation (`COMPOSE-BROKEN-DEPENDS-ON`).

use std::collections::BTreeMap;

use gf_core::{ComposeViolation, LangRuleFinding, Span};

use super::anchors_merge::ResolvedYaml;
use super::compose_resources::check_compose_resources;
use super::events_tree::YamlScalar;
use super::yaml_ast::is_extension_key;

struct DepEdge {
    target: String,
    requires_healthy: bool,
    span: Span,
}

struct ServiceNode {
    has_healthcheck: bool,
    deps: Vec<DepEdge>,
}

fn has_active_healthcheck(svc: &ResolvedYaml) -> bool {
    let Some(hc) = svc.get("healthcheck") else {
        return false;
    };
    let disabled = hc
        .get("disable")
        .and_then(ResolvedYaml::as_scalar)
        .is_some_and(|s| s.value.eq_ignore_ascii_case("true"));
    !disabled && hc.get("test").is_some()
}

fn extract_depends_on(svc: &ResolvedYaml) -> Vec<DepEdge> {
    let Some(dep_node) = svc.get("depends_on") else {
        return Vec::new();
    };
    match dep_node {
        ResolvedYaml::Sequence { items, .. } => items
            .iter()
            .filter_map(ResolvedYaml::as_scalar)
            .map(|s| DepEdge {
                target: s.value.clone(),
                requires_healthy: false,
                span: s.span,
            })
            .collect(),
        ResolvedYaml::Mapping { entries, .. } => entries
            .iter()
            .map(|(k, v)| {
                let requires_healthy = v
                    .get("condition")
                    .and_then(ResolvedYaml::as_scalar)
                    .is_some_and(|c| c.value == "service_healthy");
                DepEdge {
                    target: k.value.clone(),
                    requires_healthy,
                    span: k.span,
                }
            })
            .collect(),
        ResolvedYaml::Scalar(_) => Vec::new(),
    }
}

struct CycleSearch<'a> {
    graph: &'a BTreeMap<String, ServiceNode>,
    state: BTreeMap<&'a str, u8>,
    trail: Vec<&'a str>,
}

fn detect_dep_cycles(graph: &BTreeMap<String, ServiceNode>, out: &mut Vec<LangRuleFinding>) {
    let mut search = CycleSearch {
        graph,
        state: BTreeMap::new(),
        trail: Vec::new(),
    };
    for start in graph.keys() {
        if !search.state.contains_key(start.as_str()) {
            dfs_dep_cycle(start, &mut search, out);
        }
    }
}

fn dfs_dep_cycle<'a>(cur: &'a str, search: &mut CycleSearch<'a>, out: &mut Vec<LangRuleFinding>) {
    search.state.insert(cur, 1);
    search.trail.push(cur);
    if let Some(node) = search.graph.get(cur) {
        for edge in &node.deps {
            let Some((target_key, _)) = search.graph.get_key_value(edge.target.as_str()) else {
                continue;
            };
            match search.state.get(target_key.as_str()).copied() {
                Some(1) => out.push(LangRuleFinding::new(
                    ComposeViolation::BrokenDependsOnGraph,
                    edge.span,
                    format!(
                        "`depends_on` cycle detected: {} -> {target_key}",
                        search.trail.join(" -> ")
                    ),
                )),
                None => dfs_dep_cycle(target_key, search, out),
                Some(_) => {}
            }
        }
    }
    search.trail.pop();
    search.state.insert(cur, 2);
}

fn validate_single_dep_edge(
    svc_name: &str,
    edge: &DepEdge,
    graph: &BTreeMap<String, ServiceNode>,
    out: &mut Vec<LangRuleFinding>,
) {
    match graph.get(&edge.target) {
        None => out.push(LangRuleFinding::new(
            ComposeViolation::BrokenDependsOnGraph,
            edge.span,
            format!(
                "service `{svc_name}` depends on undefined service `{}`",
                edge.target
            ),
        )),
        Some(target_node) if edge.requires_healthy && !target_node.has_healthcheck => {
            out.push(LangRuleFinding::new(
                ComposeViolation::BrokenDependsOnGraph,
                edge.span,
                format!(
                    "service `{svc_name}` awaits `service_healthy` on `{}` which lacks an active `healthcheck:`",
                    edge.target
                ),
            ));
        }
        Some(_) => {}
    }
}

fn check_depends_on_graph(
    services: &[(YamlScalar, ResolvedYaml)],
    is_override: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut graph: BTreeMap<String, ServiceNode> = BTreeMap::new();
    for (k, v) in services {
        if !is_extension_key(&k.value) {
            graph.insert(
                k.value.clone(),
                ServiceNode {
                    has_healthcheck: has_active_healthcheck(v),
                    deps: extract_depends_on(v),
                },
            );
        }
    }
    if !is_override {
        for (svc_name, node) in &graph {
            for edge in &node.deps {
                validate_single_dep_edge(svc_name, edge, &graph, out);
            }
        }
    }
    detect_dep_cycles(&graph, out);
}

/// Validates `depends_on` graph invariants and resource declarations across the Compose root.
pub fn analyze_compose_topology(
    root: &ResolvedYaml,
    services: &[(YamlScalar, ResolvedYaml)],
    is_override: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    check_depends_on_graph(services, is_override, out);
    if !is_override {
        check_compose_resources(root, services, out);
    }
}
