//! Safe `&anchor` / `*alias` expansion (`YAML-ANCHOR-BOMB` guard) and `<<` merge-key
//! resolution (`YAML-DUPLICATE-KEY`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{LangRuleFinding, Span, YamlViolation};

pub use super::yaml_ast::ResolvedYaml;
use super::yaml_ast::{RawYamlStreamDoc, YamlNode, YamlScalar, YamlStyle};

const MAX_ALIAS_DEPTH: usize = 16;
const MAX_EXPANDED_NODES: usize = 2_048;

struct ExpandCtx<'a> {
    anchors: &'a BTreeMap<usize, YamlNode>,
    expanded_nodes: usize,
    bomb_reported: bool,
    findings: &'a mut Vec<LangRuleFinding>,
}

impl ExpandCtx<'_> {
    fn charge_alias_node(&mut self, span: Span, alias_depth: usize) -> bool {
        if self.bomb_reported {
            return false;
        }
        if alias_depth > MAX_ALIAS_DEPTH {
            self.report_bomb(
                span,
                format!("alias nesting depth {alias_depth} exceeds maximum {MAX_ALIAS_DEPTH}"),
            );
            return false;
        }
        self.expanded_nodes = self.expanded_nodes.saturating_add(1);
        if self.expanded_nodes > MAX_EXPANDED_NODES {
            self.report_bomb(
                span,
                format!(
                    "alias expansion materialized > {MAX_EXPANDED_NODES} nodes (Billion Laughs guard)"
                ),
            );
            return false;
        }
        true
    }

    fn report_bomb(&mut self, span: Span, msg: String) {
        self.bomb_reported = true;
        self.findings
            .push(LangRuleFinding::new(YamlViolation::AnchorBomb, span, msg));
    }
}

fn key_as_scalar(node: &YamlNode) -> YamlScalar {
    match node {
        YamlNode::Scalar(s) => s.clone(),
        other => YamlScalar {
            value: String::from("<complex-key>"),
            style: YamlStyle::Plain,
            has_tag: false,
            span: other.span(),
        },
    }
}

fn is_merge_key(k: &YamlScalar) -> bool {
    k.style == YamlStyle::Plain && !k.has_tag && k.value == "<<"
}

fn collect_merge_mappings<'a>(
    val: &'a ResolvedYaml,
    out: &mut Vec<&'a [(YamlScalar, ResolvedYaml)]>,
) {
    match val {
        ResolvedYaml::Mapping { entries, .. } => out.push(entries),
        ResolvedYaml::Sequence { items, .. } => {
            for item in items {
                if let ResolvedYaml::Mapping { entries, .. } = item {
                    out.push(entries);
                }
            }
        }
        ResolvedYaml::Scalar(_) => {}
    }
}

struct MergeAcc<'a> {
    explicit_seen: &'a BTreeSet<String>,
    inherited_seen: BTreeSet<String>,
    explicit_out: &'a mut Vec<(YamlScalar, ResolvedYaml)>,
    findings: Option<&'a mut Vec<LangRuleFinding>>,
}

impl MergeAcc<'_> {
    fn absorb_entry(&mut self, merge_span: Span, mk: &YamlScalar, mval: &ResolvedYaml) {
        if self.explicit_seen.contains(&mk.value) {
            return;
        }
        if !self.inherited_seen.insert(mk.value.clone()) {
            if let Some(out_f) = self.findings.as_deref_mut() {
                out_f.push(LangRuleFinding::new(
                    YamlViolation::DuplicateKey,
                    merge_span,
                    format!(
                        "conflicting key `{}` across merged `<<` anchors without explicit override",
                        mk.value
                    ),
                ));
            }
            return;
        }
        self.explicit_out.push((mk.clone(), mval.clone()));
    }
}

fn apply_inherited_merges(
    merge_values: &[(Span, ResolvedYaml)],
    explicit_seen: &BTreeSet<String>,
    explicit_out: &mut Vec<(YamlScalar, ResolvedYaml)>,
    findings: Option<&mut Vec<LangRuleFinding>>,
) {
    let mut acc = MergeAcc {
        explicit_seen,
        inherited_seen: BTreeSet::new(),
        explicit_out,
        findings,
    };
    for (merge_span, mv) in merge_values {
        let mut maps = Vec::new();
        collect_merge_mappings(mv, &mut maps);
        for map_entries in maps {
            for (mk, mval) in map_entries {
                acc.absorb_entry(*merge_span, mk, mval);
            }
        }
    }
}

fn resolve_mapping(
    entries: &[(YamlNode, YamlNode)],
    span: Span,
    alias_depth: usize,
    ctx: &mut ExpandCtx<'_>,
) -> ResolvedYaml {
    let mut explicit_seen: BTreeSet<String> = BTreeSet::new();
    let mut explicit_out: Vec<(YamlScalar, ResolvedYaml)> = Vec::new();
    let mut merge_values: Vec<(Span, ResolvedYaml)> = Vec::new();
    let mut merge_key_seen = false;

    for (raw_k, raw_v) in entries {
        let k = key_as_scalar(raw_k);
        let resolved_v = expand_node(raw_v, alias_depth, ctx);
        if is_merge_key(&k) {
            if merge_key_seen && alias_depth == 0 {
                ctx.findings.push(LangRuleFinding::new(
                    YamlViolation::DuplicateKey,
                    k.span,
                    "duplicate `<<` merge key in the same mapping; combine anchors via `<<: [*a, *b]`",
                ));
            }
            merge_key_seen = true;
            merge_values.push((k.span, resolved_v));
            continue;
        }
        if !explicit_seen.insert(k.value.clone()) && alias_depth == 0 {
            ctx.findings.push(LangRuleFinding::new(
                YamlViolation::DuplicateKey,
                k.span,
                format!("duplicate key `{}` in YAML mapping", k.value),
            ));
            continue;
        }
        explicit_out.push((k, resolved_v));
    }

    apply_inherited_merges(
        &merge_values,
        &explicit_seen,
        &mut explicit_out,
        (alias_depth == 0).then_some(&mut *ctx.findings),
    );
    ResolvedYaml::Mapping {
        entries: explicit_out,
        span,
    }
}

fn expand_node(node: &YamlNode, alias_depth: usize, ctx: &mut ExpandCtx<'_>) -> ResolvedYaml {
    if alias_depth > 0 && !ctx.charge_alias_node(node.span(), alias_depth) {
        return ResolvedYaml::Scalar(YamlScalar {
            value: String::new(),
            style: YamlStyle::Plain,
            has_tag: false,
            span: node.span(),
        });
    }
    match node {
        YamlNode::Scalar(s) => ResolvedYaml::Scalar(s.clone()),
        YamlNode::Sequence { items, span } => ResolvedYaml::Sequence {
            items: items
                .iter()
                .map(|it| expand_node(it, alias_depth, ctx))
                .collect(),
            span: *span,
        },
        YamlNode::Mapping { entries, span } => resolve_mapping(entries, *span, alias_depth, ctx),
        YamlNode::Alias { target_id, span } => {
            let Some(target) = ctx.anchors.get(target_id) else {
                return ResolvedYaml::Scalar(YamlScalar {
                    value: String::new(),
                    style: YamlStyle::Plain,
                    has_tag: false,
                    span: *span,
                });
            };
            expand_node(target, alias_depth + 1, ctx)
        }
    }
}

/// Resolves aliases and `<<` merges for a single [`RawYamlStreamDoc`].
pub fn resolve_yaml_document(
    doc: &RawYamlStreamDoc,
    findings: &mut Vec<LangRuleFinding>,
) -> Option<ResolvedYaml> {
    let root = doc.root.as_ref()?;
    let mut ctx = ExpandCtx {
        anchors: &doc.anchors,
        expanded_nodes: 0,
        bomb_reported: false,
        findings,
    };
    Some(expand_node(root, 0, &mut ctx))
}
