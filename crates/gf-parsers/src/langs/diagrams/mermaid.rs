//! Mermaid v11 structural, complexity, and anti-XSS analyzer (`MERMAID-*`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, MermaidViolation, Span};

const MAX_DEFAULT_NODES: usize = 60;

#[rustfmt::skip]
const MERMAID_HEADERS: &[&str] = &[
    "flowchart", "graph", "sequencediagram", "classdiagram", "statediagram", "statediagram-v2",
    "erdiagram", "gantt", "journey", "gitgraph", "pie", "quadrantchart", "requirementdiagram",
    "c4context", "c4container", "c4component", "c4dynamic", "c4deployment", "mindmap", "timeline",
    "zenuml", "sankey-beta", "xychart-beta", "block-beta", "packet-beta", "kanban",
    "architecture-beta",
];

#[rustfmt::skip]
const SEQ_BLOCKS: &[&str] = &["alt ", "opt ", "loop ", "par ", "critical ", "break ", "rect ", "box "];

fn push_mmd(out: &mut Vec<LangRuleFinding>, v: MermaidViolation, ln: u32, msg: impl Into<String>) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    out.push(LangRuleFinding::new(v, sp, msg.into()));
}

fn has_unsafe_click(low: &str) -> bool {
    let trimmed = low.trim_start();
    trimmed.starts_with("click ")
        && ["javascript:", "vbscript:", "data:", " call "]
            .iter()
            .any(|p| trimmed.contains(p))
}

fn has_raw_xss_html(line: &str, low: &str) -> bool {
    ["<script", "<iframe", "<object"]
        .iter()
        .any(|t| low.contains(t))
        || line.contains("onerror=")
}

fn check_security_line(line: &str, low: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    let compact: String = low
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\'' | '"'))
        .collect();
    if compact.contains("securitylevel:loose") || compact.contains("securitylevel:antiscript") {
        push_mmd(
            out,
            MermaidViolation::UnsafeSecurityOrXss,
            ln,
            "Mermaid `securityLevel` must stay `'strict'`, never `'loose'`/`'antiscript'`",
        );
    }
    if has_unsafe_click(low) {
        push_mmd(
            out,
            MermaidViolation::UnsafeSecurityOrXss,
            ln,
            "unsafe Mermaid `click` directive (`javascript:`/`call`) enables XSS",
        );
    }
    if has_raw_xss_html(line, low) {
        push_mmd(
            out,
            MermaidViolation::UnsafeSecurityOrXss,
            ln,
            "raw HTML `<script>`/`<iframe>`/`on*=` inside Mermaid label enables XSS",
        );
    }
}

fn check_reserved_end_node(trimmed: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    for arrow in ["-->", "---", "-.->", "==>", "-->|", "--->", "<-->"] {
        if let Some((lhs, rhs)) = trimmed.split_once(arrow) {
            let l_tok = lhs.split_whitespace().last().unwrap_or("");
            let r_tok = rhs.split_whitespace().next().unwrap_or("");
            if l_tok == "end" || r_tok == "end" {
                push_mmd(
                    out,
                    MermaidViolation::ReservedEndNode,
                    ln,
                    "unquoted lowercase `end` node breaks Mermaid flowchart parsing; use `END_NODE[\"end\"]`",
                );
                return;
            }
        }
    }
}

const fn bracket_delta(ch: char) -> (i32, i32) {
    match ch {
        '[' => (1, 0),
        ']' => (-1, 0),
        '(' => (0, 1),
        ')' => (0, -1),
        _ => (0, 0),
    }
}

fn has_balanced_quotes_and_brackets(trimmed: &str) -> bool {
    let mut in_q = false;
    let (mut sq, mut par) = (0i32, 0i32);
    for ch in trimmed.chars() {
        if ch == '"' {
            in_q = !in_q;
            continue;
        }
        if in_q {
            continue;
        }
        let (ds, dp) = bracket_delta(ch);
        sq += ds;
        par += dp;
        if sq < 0 || par < 0 {
            return false;
        }
    }
    !in_q && sq == 0 && par == 0
}

fn insert_leading_id(side: &str, nodes: &mut BTreeSet<String>) {
    let id: String = side
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if !id.is_empty() {
        nodes.insert(id);
    }
}

fn collect_flowchart_nodes(trimmed: &str, nodes: &mut BTreeSet<String>) {
    for arrow in ["-->", "---", "-.->", "==>"] {
        if let Some((lhs, rhs)) = trimmed.split_once(arrow) {
            insert_leading_id(lhs, nodes);
            insert_leading_id(rhs, nodes);
        }
    }
}

struct MmdState {
    in_frontmatter: bool,
    header_kind: Option<String>,
    block_stack: Vec<u32>,
    nodes: BTreeSet<String>,
}

impl MmdState {
    fn step_body_line(&mut self, trimmed: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
        let low = trimmed.to_ascii_lowercase();
        let is_flow = matches!(self.header_kind.as_deref(), Some("flowchart" | "graph"));
        let is_seq = self.header_kind.as_deref() == Some("sequencediagram");
        if (is_flow && low.starts_with("subgraph"))
            || (is_seq && SEQ_BLOCKS.iter().any(|b| low.starts_with(b)))
        {
            self.block_stack.push(ln);
        } else if low == "end" {
            self.block_stack.pop();
        }
        if is_flow {
            check_reserved_end_node(trimmed, ln, out);
            collect_flowchart_nodes(trimmed, &mut self.nodes);
            if !has_balanced_quotes_and_brackets(trimmed) {
                push_mmd(
                    out,
                    MermaidViolation::UnclosedBlockOrShape,
                    ln,
                    String::from("unbalanced quotes or shape brackets in Mermaid flowchart node"),
                );
            }
        }
    }

    fn finalize(self, base_line: u32, out: &mut Vec<LangRuleFinding>) {
        if self.header_kind.is_none() {
            push_mmd(
                out,
                MermaidViolation::UnknownOrEmptyDiagram,
                base_line,
                String::from("empty Mermaid diagram"),
            );
        }
        for open_ln in self.block_stack {
            push_mmd(
                out,
                MermaidViolation::UnclosedBlockOrShape,
                open_ln,
                String::from(
                    "unclosed Mermaid block (`subgraph`/`alt`/`opt`/`loop`) missing `end`",
                ),
            );
        }
        if self.nodes.len() > MAX_DEFAULT_NODES {
            let msg = format!(
                "Mermaid diagram declares {} nodes (max {MAX_DEFAULT_NODES})",
                self.nodes.len()
            );
            push_mmd(out, MermaidViolation::SpaghettiComplexity, base_line, msg);
        }
    }
    fn try_parse_header(&mut self, low: &str, ln: u32, out: &mut Vec<LangRuleFinding>) -> bool {
        let first = low.split_whitespace().next().unwrap_or("");
        if MERMAID_HEADERS.contains(&first) {
            self.header_kind = Some(String::from(first));
            return true;
        }
        push_mmd(
            out,
            MermaidViolation::UnknownOrEmptyDiagram,
            ln,
            format!("unknown Mermaid v11 diagram header `{first}`"),
        );
        false
    }
    fn step_line(&mut self, line: &str, ln: u32, out: &mut Vec<LangRuleFinding>) -> bool {
        let trimmed = line.trim();
        let low = trimmed.to_ascii_lowercase();
        check_security_line(line, &low, ln, out);
        if trimmed == "---" && self.header_kind.is_none() {
            self.in_frontmatter = !self.in_frontmatter;
            return true;
        }
        if self.in_frontmatter || trimmed.is_empty() || trimmed.starts_with("%%") {
            return true;
        }
        if self.header_kind.is_none() {
            return self.try_parse_header(&low, ln, out);
        }
        self.step_body_line(trimmed, ln, out);
        true
    }
}

/// Validates a Mermaid v11 diagram block starting at 1-based `base_line`.
pub fn analyze_mermaid_block(source: &str, base_line: u32, out: &mut Vec<LangRuleFinding>) {
    let mut st = MmdState {
        in_frontmatter: false,
        header_kind: None,
        block_stack: Vec::new(),
        nodes: BTreeSet::new(),
    };
    for (idx, line) in source.lines().enumerate() {
        let ln = base_line.saturating_add(u32::try_from(idx).unwrap_or(0));
        if !st.step_line(line, ln, out) {
            return;
        }
    }
    st.finalize(base_line, out);
}
