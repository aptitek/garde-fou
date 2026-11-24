//! Workspace-wide Type-2 (`BLAKE3`) and Type-3 (`MinHash` 5-gram shingle) AST clone
//! detection (`GF020`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, RepoPath, Severity, Span, UastDocument, Validation,
    classify_path,
};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct CloneSite<'a> {
    path: &'a RepoPath,
    span: Span,
    name: &'a str,
    effective_lines: u32,
    ast_node_count: u32,
    shingle_sketch: Option<[u64; 8]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct SiteKey<'a> {
    path: &'a RepoPath,
    span: Span,
}

fn emit_clone_group(sites: &[CloneSite<'_>], min_lines: u32, out: &mut Vec<Diagnostic>) {
    let origin = sites[0];
    for (idx, site) in sites.iter().enumerate() {
        let peer = if idx == 0 { sites[1] } else { origin };
        out.push(Diagnostic {
            rule_id: String::from("GF020"),
            severity: Severity::Error,
            message: format!(
                "Type-2 AST clone: callable `{}` duplicates `{}` at `{}:{}`",
                site.name,
                peer.name,
                peer.path,
                peer.span.start_line()
            ),
            remediation: String::from(
                "Extract the duplicated control-flow structure into a shared generic or parameterized function; never rename variables or literals cosmetically to mask copy-paste.",
            ),
            measured: site.effective_lines,
            threshold: min_lines,
            path: site.path.clone(),
            span: site.span,
        });
    }
}

fn type3_similarity_pct(a: CloneSite<'_>, b: CloneSite<'_>, min_sim_pct: u32) -> Option<u32> {
    let (sa, sb) = (a.shingle_sketch?, b.shingle_sketch?);
    let min_n = a.ast_node_count.min(b.ast_node_count);
    let max_n = a.ast_node_count.max(b.ast_node_count).max(1);
    if min_n.saturating_mul(100) / max_n < min_sim_pct {
        return None;
    }
    let matching =
        u32::try_from(sa.iter().zip(sb.iter()).filter(|(x, y)| x == y).count()).unwrap_or(0);
    let sim_pct = matching.saturating_mul(100) / 8;
    (sim_pct >= min_sim_pct).then_some(sim_pct)
}

fn build_type3_diag(
    site: CloneSite<'_>,
    peer: CloneSite<'_>,
    sim_pct: u32,
    min_sim_pct: u32,
) -> Diagnostic {
    Diagnostic {
        rule_id: String::from("GF020"),
        severity: Severity::Error,
        message: format!(
            "Type-3 near-miss AST clone ({sim_pct}% structural similarity): callable `{}` mirrors `{}` at `{}:{}`",
            site.name,
            peer.name,
            peer.path,
            peer.span.start_line()
        ),
        remediation: String::from(
            "Extract the shared control-flow skeleton into a parameterized helper function; injecting dummy statements or logs into copied code does not eliminate structural duplication.",
        ),
        measured: sim_pct,
        threshold: min_sim_pct,
        path: site.path.clone(),
        span: site.span,
    }
}

fn detect_type3_clones(candidates: &[CloneSite<'_>], min_sim_pct: u32, out: &mut Vec<Diagnostic>) {
    let mut emitted: BTreeSet<SiteKey<'_>> = BTreeSet::new();
    for (i, &a) in candidates.iter().enumerate() {
        for &b in &candidates[i + 1..] {
            let Some(sim_pct) = type3_similarity_pct(a, b, min_sim_pct) else {
                continue;
            };
            if emitted.insert(SiteKey {
                path: a.path,
                span: a.span,
            }) {
                out.push(build_type3_diag(a, b, sim_pct, min_sim_pct));
            }
            if emitted.insert(SiteKey {
                path: b.path,
                span: b.span,
            }) {
                out.push(build_type3_diag(b, a, sim_pct, min_sim_pct));
            }
        }
    }
}

fn collect_eligible_clones<'a>(
    docs: &'a [UastDocument],
    min_lines: u32,
    min_nodes: u32,
    buckets: &mut BTreeMap<u128, Vec<CloneSite<'a>>>,
) -> Vec<CloneSite<'a>> {
    let mut all_eligible = Vec::new();
    for doc in docs {
        for callable in &doc.callables {
            if callable.effective_lines < min_lines || callable.ast_node_count < min_nodes {
                continue;
            }
            let site = CloneSite {
                path: &doc.path,
                span: callable.span,
                name: &callable.name,
                effective_lines: callable.effective_lines,
                ast_node_count: callable.ast_node_count,
                shingle_sketch: callable.shingle_sketch,
            };
            all_eligible.push(site);
            if let Some(hash) = callable.structural_hash {
                buckets.entry(hash).or_default().push(site);
            }
        }
    }
    all_eligible
}

/// Detects Type-2 (`BLAKE3`) and Type-3 (`MinHash`) AST clones (`GF020`) across `docs`.
pub fn check_type2_clones(docs: &[UastDocument], config: &Config) -> Validation<(), Diagnostic> {
    let min_lines = config.limits.min_clone_lines;
    let mut buckets: BTreeMap<u128, Vec<CloneSite<'_>>> = BTreeMap::new();
    let all_eligible =
        collect_eligible_clones(docs, min_lines, config.limits.min_clone_nodes, &mut buckets);

    let mut exact_matched: BTreeSet<SiteKey<'_>> = BTreeSet::new();
    let mut diags = Vec::new();
    for mut sites in buckets.into_values() {
        if sites.len() >= 2 {
            sites.sort();
            for s in &sites {
                exact_matched.insert(SiteKey {
                    path: s.path,
                    span: s.span,
                });
            }
            emit_clone_group(&sites, min_lines, &mut diags);
        }
    }

    let mut ungrouped: Vec<CloneSite<'_>> = all_eligible
        .into_iter()
        .filter(|s| {
            s.shingle_sketch.is_some()
                && classify_path(s.path, &config.domains) != DomainKind::Tests
                && !exact_matched.contains(&SiteKey {
                    path: s.path,
                    span: s.span,
                })
        })
        .collect();
    ungrouped.sort();
    detect_type3_clones(
        &ungrouped,
        config.limits.min_clone_similarity_pct,
        &mut diags,
    );
    Validation::<(), Diagnostic>::from_errors(diags)
}
