//! `PlantUML` structural, anti-SSRF, local include, and anti-XSS analyzer (`PLANTUML-*`).

use gf_core::{LangRuleFinding, PlantUmlViolation, Span};

#[rustfmt::skip]
const BLOCK_OPENERS: &[&str] = &["alt ", "opt ", "loop ", "par ", "break ", "critical ", "group "];

fn push_puml(out: &mut Vec<LangRuleFinding>, v: PlantUmlViolation, ln: u32, msg: String) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    out.push(LangRuleFinding::new(v, sp, msg));
}

fn is_unsafe_include_target(target: &str) -> bool {
    let tlow = target.to_ascii_lowercase();
    let is_remote = ["http://", "https://", "ftp://"]
        .iter()
        .any(|p| tlow.starts_with(p))
        || tlow.contains("](http");
    let is_escape = target.starts_with('/')
        || target.starts_with("../")
        || target.contains("/../")
        || target.contains("..\\")
        || (target.len() >= 2 && target.as_bytes()[1] == b':');
    is_remote || is_escape
}

fn check_include_security(trimmed: &str, low: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    if low.starts_with("!includeurl") || low.starts_with("!import ") {
        push_puml(
            out,
            PlantUmlViolation::SsrfOrLocalLeak,
            ln,
            String::from("`!includeurl` / `!import` enables SSRF or arbitrary file inclusion"),
        );
        return;
    }
    let Some(rest) = ["!include_many ", "!include_once ", "!include "]
        .iter()
        .find_map(|p| {
            low.strip_prefix(p)
                .map(|r| &trimmed[trimmed.len() - r.len()..])
        })
    else {
        return;
    };
    let target = rest.trim().trim_matches('"');
    if !(target.starts_with('<') && target.ends_with('>')) && is_unsafe_include_target(target) {
        push_puml(
            out,
            PlantUmlViolation::SsrfOrLocalLeak,
            ln,
            format!(
                "unsafe PlantUML `!include {target}` (remote URL, absolute path, or `..` traversal)"
            ),
        );
    }
}

fn check_preproc_and_xss(low: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    if ["%getenv(", "%file_exists(", "%dirpath("]
        .iter()
        .any(|f| low.contains(f))
        || low.starts_with("!log ")
    {
        push_puml(
            out,
            PlantUmlViolation::SsrfOrLocalLeak,
            ln,
            String::from(
                "PlantUML preprocessor function (`%getenv`/`%file_exists`/`!log`) leaks host state",
            ),
        );
    }
    if low.contains("[[javascript:") || low.contains("[[vbscript:") || low.contains("[[data:") {
        push_puml(
            out,
            PlantUmlViolation::XssLink,
            ln,
            String::from("PlantUML `[[javascript:...]]` / `[[data:...]]` hyperlink enables XSS"),
        );
    }
}

struct PumlState {
    in_block_comment: bool,
    start_kind: Option<(String, u32)>,
    brace_depth: i32,
    seq_stack: Vec<u32>,
}

impl PumlState {
    fn skip_comment(&mut self, trimmed: &str) -> bool {
        if self.in_block_comment {
            if trimmed.contains("'/") {
                self.in_block_comment = false;
            }
            return true;
        }
        if trimmed.starts_with("/'") {
            self.in_block_comment = !trimmed.contains("'/");
            return true;
        }
        trimmed.is_empty() || trimmed.starts_with('\'')
    }

    fn handle_start_end(&mut self, low: &str, ln: u32, out: &mut Vec<LangRuleFinding>) -> bool {
        if let Some(kind) = low.strip_prefix("@start") {
            let k = kind.split_whitespace().next().unwrap_or("uml");
            self.start_kind = Some((String::from(k), ln));
            return true;
        }
        let Some(kind) = low.strip_prefix("@end") else {
            return false;
        };
        let k = kind.split_whitespace().next().unwrap_or("uml");
        if !matches!(self.start_kind.take(), Some((open_k, _)) if open_k == k) {
            push_puml(
                out,
                PlantUmlViolation::UnbalancedStructure,
                ln,
                format!("mismatched `@end{k}` delimiter"),
            );
        }
        true
    }

    fn step_line(&mut self, trimmed: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
        if self.skip_comment(trimmed) {
            return;
        }
        let low = trimmed.to_ascii_lowercase();
        if self.handle_start_end(&low, ln, out) {
            return;
        }
        check_include_security(trimmed, &low, ln, out);
        check_preproc_and_xss(&low, ln, out);
        if BLOCK_OPENERS.iter().any(|b| low.starts_with(b)) {
            self.seq_stack.push(ln);
        } else if low == "end" {
            self.seq_stack.pop();
        }
        self.brace_depth += i32::from(trimmed.ends_with('{')) - i32::from(trimmed == "}");
    }
}

/// Validates a `PlantUML` diagram block starting at 1-based `base_line`.
pub fn analyze_plantuml_block(source: &str, base_line: u32, out: &mut Vec<LangRuleFinding>) {
    let mut st = PumlState {
        in_block_comment: false,
        start_kind: None,
        brace_depth: 0,
        seq_stack: Vec::new(),
    };
    for (idx, line) in source.lines().enumerate() {
        let ln = base_line.saturating_add(u32::try_from(idx).unwrap_or(0));
        st.step_line(line.trim(), ln, out);
    }
    if let Some((k, ln)) = st.start_kind {
        push_puml(
            out,
            PlantUmlViolation::UnbalancedStructure,
            ln,
            format!("unclosed `@start{k}` missing `@end{k}`"),
        );
    }
    for ln in st.seq_stack {
        push_puml(
            out,
            PlantUmlViolation::UnbalancedStructure,
            ln,
            String::from("unclosed PlantUML `alt`/`opt`/`loop`/`group` block missing `end`"),
        );
    }
    if st.brace_depth != 0 {
        push_puml(
            out,
            PlantUmlViolation::UnbalancedStructure,
            base_line,
            String::from("unbalanced `{ ... }` container braces in PlantUML diagram"),
        );
    }
}
