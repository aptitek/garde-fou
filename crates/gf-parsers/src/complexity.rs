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
    /// `McCabe` cyclomatic complexity (`>= 1`).
    pub cyclomatic: u32,
    /// Cognitive complexity including nesting depth penalties.
    pub cognitive: u32,
    /// Maximum control-flow nesting depth reached.
    pub max_nesting: u32,
}

struct Accumulator<'a> {
    source: &'a str,
    rules: &'a ComplexityRules,
    extra_cyclomatic: u32,
    cognitive: u32,
    max_nesting: u32,
}

/// Computes cyclomatic complexity, cognitive complexity, and maximum nesting depth for `body`.
pub fn compute_metrics(body: Node<'_>, source: &str, rules: &ComplexityRules) -> ComputedMetrics {
    let mut acc = Accumulator {
        source,
        rules,
        extra_cyclomatic: 0,
        cognitive: 0,
        max_nesting: 0,
    };
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        acc.visit_node(child, 0);
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

impl Accumulator<'_> {
    fn visit_node(&mut self, node: Node<'_>, depth: u32) {
        if self.rules.nested_fn_nodes.contains(&node.kind()) {
            return;
        }
        let next_depth = self.classify_and_score(node, depth);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, next_depth);
        }
    }

    fn classify_and_score(&mut self, node: Node<'_>, depth: u32) -> u32 {
        let kind = node.kind();
        if is_else_if(node, self.rules) || self.rules.flat_branches.contains(&kind) {
            self.extra_cyclomatic = self.extra_cyclomatic.saturating_add(1);
            self.cognitive = self.cognitive.saturating_add(1);
            return depth;
        }
        if self.rules.nesting_branches.contains(&kind) {
            let next = depth.saturating_add(1);
            self.extra_cyclomatic = self.extra_cyclomatic.saturating_add(1);
            self.cognitive = self.cognitive.saturating_add(1).saturating_add(depth);
            self.max_nesting = self.max_nesting.max(next);
            return next;
        }
        if self.rules.switch_nodes.contains(&kind) {
            let next = depth.saturating_add(1);
            self.extra_cyclomatic = self
                .extra_cyclomatic
                .saturating_add(count_switch_arms(node, self.rules));
            self.cognitive = self.cognitive.saturating_add(1).saturating_add(depth);
            self.max_nesting = self.max_nesting.max(next);
            return next;
        }
        if self.rules.bool_expr_nodes.contains(&kind)
            && has_short_circuit_op(node, self.source, self.rules)
        {
            self.extra_cyclomatic = self.extra_cyclomatic.saturating_add(1);
            self.cognitive = self.cognitive.saturating_add(1);
        }
        depth
    }
}
