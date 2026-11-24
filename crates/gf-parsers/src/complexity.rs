//! Declarative cyclomatic complexity, cognitive complexity, and nesting depth calculator.

use tree_sitter::Node;

use crate::types::node_text;

/// Language-specific CST node kinds participating in control-flow complexity.
pub struct ComplexityRules {
    /// Control-flow nodes that increment cyclomatic (+1), cognitive (+1 + depth), and depth (+1).
    pub nesting_branches: &'static [&'static str],
    /// Flat continuation branches (e.g. Python `elif_clause`) adding +1 cyclomatic and +1 cognitive.
    pub flat_branches: &'static [&'static str],
    /// Multi-way `match` / `switch` containers adding cognitive (+1 + depth) and depth (+1).
    pub switch_nodes: &'static [&'static str],
    /// Case / arm node kinds inside `switch_nodes`.
    pub case_arm_nodes: &'static [&'static str],
    /// Whether `switch_nodes` include an exhaustive/wildcard arm (`match` = true, `switch` = false).
    pub subtract_first_arm: bool,
    /// Binary expression node kinds that may hold short-circuit boolean operators.
    pub bool_expr_nodes: &'static [&'static str],
    /// Short-circuit boolean operator tokens (`&&`, `||`, `??`, `and`, `or`).
    pub bool_ops: &'static [&'static str],
    /// Nested standalone function definitions excluded from the enclosing callable's score.
    pub nested_fn_nodes: &'static [&'static str],
    /// `else` wrapper kinds used to recognize `else if` chains without double nesting.
    pub else_wrapper_nodes: &'static [&'static str],
}

/// Aggregated complexity metrics for a single callable body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputedMetrics {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub max_nesting: u32,
}

#[derive(Default)]
struct Accumulator {
    extra_cyclomatic: u32,
    cognitive: u32,
    max_nesting: u32,
}

/// Computes cyclomatic complexity, cognitive complexity, and maximum nesting depth for `body`.
pub fn compute_metrics(body: Node<'_>, source: &str, rules: &ComplexityRules) -> ComputedMetrics {
    let mut acc = Accumulator::default();
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        visit_node(child, source, rules, (0, &mut acc));
    }
    ComputedMetrics {
        cyclomatic: acc.extra_cyclomatic.saturating_add(1),
        cognitive: acc.cognitive,
        max_nesting: acc.max_nesting,
    }
}

fn is_else_if(node: Node<'_>, rules: &ComplexityRules) -> bool {
    let kind = node.kind();
    if kind != "if_expression" && kind != "if_statement" {
        return false;
    }
    node.parent()
        .is_some_and(|parent| rules.else_wrapper_nodes.contains(&parent.kind()))
}

fn count_direct_case_arms(node: Node<'_>, rules: &ComplexityRules) -> u32 {
    let mut cursor = node.walk();
    let matched = node
        .children(&mut cursor)
        .filter(|c| rules.case_arm_nodes.contains(&c.kind()))
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

fn count_switch_arms(node: Node<'_>, rules: &ComplexityRules) -> u32 {
    let direct = count_direct_case_arms(node, rules);
    let mut nested = 0_u32;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !rules.case_arm_nodes.contains(&child.kind()) {
            nested = nested.saturating_add(count_direct_case_arms(child, rules));
        }
    }
    let total = direct.saturating_add(nested);
    if rules.subtract_first_arm {
        total.saturating_sub(1)
    } else {
        total
    }
}

fn has_short_circuit_op(node: Node<'_>, source: &str, rules: &ComplexityRules) -> bool {
    if let Some(op_node) = node.child_by_field_name("operator") {
        return rules.bool_ops.contains(&node_text(op_node, source));
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && rules.bool_ops.contains(&node_text(child, source)))
}

fn visit_node(
    node: Node<'_>,
    source: &str,
    rules: &ComplexityRules,
    state: (u32, &mut Accumulator),
) {
    let (depth, acc) = state;
    let kind = node.kind();
    if rules.nested_fn_nodes.contains(&kind) {
        return;
    }

    let next_depth = classify_and_score(node, source, rules, (depth, acc));
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_node(child, source, rules, (next_depth, acc));
    }
}

fn classify_and_score(
    node: Node<'_>,
    source: &str,
    rules: &ComplexityRules,
    state: (u32, &mut Accumulator),
) -> u32 {
    let (depth, acc) = state;
    let kind = node.kind();

    if is_else_if(node, rules) || rules.flat_branches.contains(&kind) {
        acc.extra_cyclomatic = acc.extra_cyclomatic.saturating_add(1);
        acc.cognitive = acc.cognitive.saturating_add(1);
        return depth;
    }
    if rules.nesting_branches.contains(&kind) {
        let next = depth.saturating_add(1);
        acc.extra_cyclomatic = acc.extra_cyclomatic.saturating_add(1);
        acc.cognitive = acc.cognitive.saturating_add(1).saturating_add(depth);
        acc.max_nesting = acc.max_nesting.max(next);
        return next;
    }
    if rules.switch_nodes.contains(&kind) {
        let next = depth.saturating_add(1);
        acc.extra_cyclomatic = acc
            .extra_cyclomatic
            .saturating_add(count_switch_arms(node, rules));
        acc.cognitive = acc.cognitive.saturating_add(1).saturating_add(depth);
        acc.max_nesting = acc.max_nesting.max(next);
        return next;
    }
    if rules.bool_expr_nodes.contains(&kind) && has_short_circuit_op(node, source, rules) {
        acc.extra_cyclomatic = acc.extra_cyclomatic.saturating_add(1);
        acc.cognitive = acc.cognitive.saturating_add(1);
    }
    depth
}
