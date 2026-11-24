//! Pure `#![no_std]` Tarjan Strongly Connected Components (`SCC`) cycle detector (`GF071`).

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::format;
use alloc::vec::Vec;

use gf_core::{Diagnostic, Span, UastDocument, Validation};

use crate::checks::{SourceLoc, rule_error};
use crate::import_resolver::WorkspaceImportResolver;

struct TarjanState {
    next_index: usize,
    indices: BTreeMap<usize, usize>,
    lowlink: BTreeMap<usize, usize>,
    stack: Vec<usize>,
    on_stack: BTreeSet<usize>,
    sccs: Vec<Vec<usize>>,
}

impl TarjanState {
    const fn new() -> Self {
        Self {
            next_index: 0,
            indices: BTreeMap::new(),
            lowlink: BTreeMap::new(),
            stack: Vec::new(),
            on_stack: BTreeSet::new(),
            sccs: Vec::new(),
        }
    }

    fn lower_lowlink(&mut self, v: usize, candidate: usize) {
        if let Some(v_low) = self.lowlink.get_mut(&v) {
            *v_low = (*v_low).min(candidate);
        }
    }

    fn visit_edge(&mut self, v: usize, w: usize, adj: &BTreeMap<usize, BTreeSet<usize>>) {
        let v_idx = self.indices.get(&v).copied().unwrap_or(0);
        if !self.indices.contains_key(&w) {
            self.strong_connect(w, adj);
            let w_low = self.lowlink.get(&w).copied().unwrap_or(v_idx);
            self.lower_lowlink(v, w_low);
        } else if self.on_stack.contains(&w) {
            let w_idx = self.indices.get(&w).copied().unwrap_or(v_idx);
            self.lower_lowlink(v, w_idx);
        }
    }

    fn pop_scc_from(&mut self, root_v: usize) {
        let mut component = Vec::new();
        while let Some(w) = self.stack.pop() {
            self.on_stack.remove(&w);
            component.push(w);
            if w == root_v {
                break;
            }
        }
        component.sort_unstable();
        self.sccs.push(component);
    }

    fn strong_connect(&mut self, v: usize, adj: &BTreeMap<usize, BTreeSet<usize>>) {
        let v_idx = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        self.indices.insert(v, v_idx);
        self.lowlink.insert(v, v_idx);
        self.stack.push(v);
        self.on_stack.insert(v);

        if let Some(neighbors) = adj.get(&v) {
            for &w in neighbors {
                self.visit_edge(v, w, adj);
            }
        }
        if self.lowlink.get(&v) == self.indices.get(&v) {
            self.pop_scc_from(v);
        }
    }
}

struct SccBfs<'a> {
    member_set: &'a BTreeSet<usize>,
    parent: &'a mut BTreeMap<usize, usize>,
    queue: VecDeque<usize>,
}

impl<'a> SccBfs<'a> {
    const fn new(member_set: &'a BTreeSet<usize>, parent: &'a mut BTreeMap<usize, usize>) -> Self {
        Self {
            member_set,
            parent,
            queue: VecDeque::new(),
        }
    }

    fn enqueue_neighbors(&mut self, from: usize, neighbors: &BTreeSet<usize>) {
        for &nxt in neighbors {
            if self.member_set.contains(&nxt) && !self.parent.contains_key(&nxt) {
                self.parent.insert(nxt, from);
                self.queue.push_back(nxt);
            }
        }
    }
}

fn bfs_last_hop_to_start(
    start: usize,
    member_set: &BTreeSet<usize>,
    adj: &BTreeMap<usize, BTreeSet<usize>>,
    parent: &mut BTreeMap<usize, usize>,
) -> Option<usize> {
    let mut bfs = SccBfs::new(member_set, parent);
    bfs.enqueue_neighbors(start, adj.get(&start)?);
    while let Some(cur) = bfs.queue.pop_front() {
        let Some(neighbors) = adj.get(&cur) else {
            continue;
        };
        if neighbors.contains(&start) {
            return Some(cur);
        }
        bfs.enqueue_neighbors(cur, neighbors);
    }
    None
}

fn shortest_cycle_in_scc(
    scc: &[usize],
    adj: &BTreeMap<usize, BTreeSet<usize>>,
) -> Option<Vec<usize>> {
    let &start = scc.first()?;
    if adj.get(&start).is_some_and(|n| n.contains(&start)) {
        return Some(alloc::vec![start, start]);
    }
    let member_set: BTreeSet<usize> = scc.iter().copied().collect();
    let mut parent = BTreeMap::new();
    let end = bfs_last_hop_to_start(start, &member_set, adj, &mut parent)?;

    let mut rev_chain = alloc::vec![start, end];
    let mut cur = end;
    while cur != start {
        cur = *parent.get(&cur)?;
        rev_chain.push(cur);
    }
    rev_chain.reverse();
    Some(rev_chain)
}

/// Computes deterministic closed cycles `[v0, ..., vk, v0]` across `0..node_count` using Tarjan's SCC.
#[must_use]
pub fn find_scc_cycles(
    node_count: usize,
    adj: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<Vec<usize>> {
    let mut state = TarjanState::new();
    for v in 0..node_count {
        if !state.indices.contains_key(&v) {
            state.strong_connect(v, adj);
        }
    }

    let mut cycles = Vec::new();
    for scc in &state.sccs {
        let self_loop = scc.len() == 1 && adj.get(&scc[0]).is_some_and(|n| n.contains(&scc[0]));
        if self_loop {
            cycles.push(alloc::vec![scc[0], scc[0]]);
        } else if scc.len() >= 2
            && let Some(cycle) = shortest_cycle_in_scc(scc, adj)
        {
            cycles.push(cycle);
        }
    }
    cycles.sort();
    cycles
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct DirectedEdge {
    from: usize,
    to: usize,
}

fn build_runtime_import_graph(
    docs: &[UastDocument],
) -> (
    BTreeMap<usize, BTreeSet<usize>>,
    BTreeMap<DirectedEdge, Span>,
) {
    let resolver = WorkspaceImportResolver::new(docs);
    let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut edge_spans: BTreeMap<DirectedEdge, Span> = BTreeMap::new();
    for (from_idx, doc) in docs.iter().enumerate() {
        for imp in doc.imports.iter().filter(|i| !i.is_type_only) {
            for to_idx in resolver.resolve(&doc.path, doc.language, imp) {
                adj.entry(from_idx).or_default().insert(to_idx);
                edge_spans
                    .entry(DirectedEdge {
                        from: from_idx,
                        to: to_idx,
                    })
                    .or_insert(imp.span);
            }
        }
    }
    (adj, edge_spans)
}

/// Evaluates inter-file runtime import DAG acyclicity (`GF071`) across the workspace.
///
/// Purely compile-time `import type` edges (`imp.is_type_only`) are ignored for runtime cycles,
/// while remaining enforced for architectural `[[boundaries]]` (`GF072`).
pub fn check_dependency_cycles(docs: &[UastDocument]) -> Validation<(), Diagnostic> {
    let (adj, edge_spans) = build_runtime_import_graph(docs);
    let mut diags = Vec::new();
    for cycle in find_scc_cycles(docs.len(), &adj) {
        let chain_str = cycle
            .iter()
            .map(|&idx| docs[idx].path.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        for pair in cycle.windows(2) {
            let edge = DirectedEdge {
                from: pair[0],
                to: pair[1],
            };
            let span = edge_spans.get(&edge).copied().unwrap_or(Span::FIRST);
            diags.push(rule_error(
                "GF071",
                format!("circular dependency cycle detected: {chain_str}"),
                "Break the dependency cycle by extracting shared types/contracts into a lower-layer leaf module so imports form a strict DAG.",
                SourceLoc::new(&docs[edge.from].path, span),
            ));
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
