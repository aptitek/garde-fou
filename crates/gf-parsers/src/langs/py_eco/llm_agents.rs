//! `LangChain` 1.x & `LangGraph` 1.x CST analyzer (`LLM-*` under `GF080`).

use gf_core::{LangRuleFinding, LlmAgentViolation};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

const DANGEROUS_LLM_TOOLS: &[&str] = &[
    "PythonREPLTool",
    "PythonAstREPLTool",
    "BashProcess",
    "ShellTool",
];

const LLM_CLIENT_CLASSES: &[&str] = &[
    "ChatOpenAI",
    "ChatAnthropic",
    "ChatGoogleGenerativeAI",
    "ChatMistralAI",
    "ChatOllama",
];

fn push_llm(out: &mut Vec<LangRuleFinding>, v: LlmAgentViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

struct LlmCall<'a> {
    node: Node<'a>,
    base: &'a str,
    args_txt: &'a str,
    compact: String,
}

fn check_llm_rce_args(call: &LlmCall<'_>, out: &mut Vec<LangRuleFinding>) {
    if call.compact.contains("allow_dangerous_code=True")
        || call.compact.contains("allow_dangerous_requests=True")
    {
        push_llm(
            out,
            LlmAgentViolation::RceAndCveVectors,
            call.node,
            String::from("`allow_dangerous_*=True` enables RCE/SSRF"),
        );
    }
    if matches!(call.base, "loads" | "load") && call.compact.contains("secrets_from_env=True") {
        push_llm(
            out,
            LlmAgentViolation::RceAndCveVectors,
            call.node,
            String::from("`secrets_from_env=True` in LangChain `load`/`loads` leaks secrets"),
        );
    }
    if call.compact.contains("template_format=\"jinja2\"")
        || call.compact.contains("template_format='jinja2'")
    {
        push_llm(
            out,
            LlmAgentViolation::RceAndCveVectors,
            call.node,
            String::from("`template_format=\"jinja2\"` enables SSTI -> RCE"),
        );
    }
}

fn check_llm_security_call(call: &LlmCall<'_>, in_code: bool, out: &mut Vec<LangRuleFinding>) {
    if in_code && DANGEROUS_LLM_TOOLS.contains(&call.base) {
        let msg = format!("unbounded code-execution tool `{}` is forbidden", call.base);
        push_llm(out, LlmAgentViolation::RceAndCveVectors, call.node, msg);
    }
    if matches!(call.base, "load_prompt" | "load_prompt_from_config") {
        let msg = format!("`{}(...)` is vulnerable to path traversal/deser", call.base);
        push_llm(out, LlmAgentViolation::RceAndCveVectors, call.node, msg);
    }
    check_llm_rce_args(call, out);
}

fn check_langgraph_budget(call: &LlmCall<'_>, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let (has_end, has_cond) = ((flags & 2) != 0, (flags & 4) != 0);
    if call.base == "compile" && !has_end {
        push_llm(
            out,
            LlmAgentViolation::LangGraphInfiniteLoopAndBudget,
            call.node,
            String::from("compiled `StateGraph` has no edge reaching `END` (`\"__end__\"`)"),
        );
    }
    if has_cond
        && matches!(call.base, "invoke" | "ainvoke" | "astream")
        && !call.args_txt.contains("recursion_limit")
    {
        let msg = format!(
            "cyclic `StateGraph.{}(...)` must pass `config={{\"recursion_limit\": N}}`",
            call.base
        );
        push_llm(
            out,
            LlmAgentViolation::LangGraphInfiniteLoopAndBudget,
            call.node,
            msg,
        );
    }
}

fn check_llm_prompt_and_budget(call: &LlmCall<'_>, flags: u8, out: &mut Vec<LangRuleFinding>) {
    if call.base == "SystemMessage"
        && (call.args_txt.contains("f\"") || call.args_txt.contains("f'"))
    {
        push_llm(
            out,
            LlmAgentViolation::SystemPromptInjection,
            call.node,
            String::from("dynamic f-string in `SystemMessage(...)` risks prompt injection"),
        );
    }
    if (flags & 1) != 0
        && LLM_CLIENT_CLASSES.contains(&call.base)
        && !call.compact.contains("timeout=")
        && !call.compact.contains("request_timeout=")
    {
        let msg = format!(
            "LLM client `{}(...)` must configure `timeout=...`",
            call.base
        );
        push_llm(
            out,
            LlmAgentViolation::LangGraphInfiniteLoopAndBudget,
            call.node,
            msg,
        );
    }
    check_langgraph_budget(call, flags, out);
}

fn walk_llm(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "call" => {
            if let Some(func) = node.child_by_field_name("function") {
                let ftxt = node_text(func, source).trim();
                let args_txt = node
                    .child_by_field_name("arguments")
                    .map_or("", |a| node_text(a, source));
                let call = LlmCall {
                    node,
                    base: ftxt.rsplit('.').next().unwrap_or(ftxt),
                    args_txt,
                    compact: args_txt.chars().filter(|c| !c.is_whitespace()).collect(),
                };
                check_llm_security_call(&call, (flags & 1) != 0, out);
                check_llm_prompt_and_budget(&call, flags, out);
            }
        }
        "tuple" => {
            let txt = node_text(node, source);
            if (txt.contains("\"system\"") || txt.contains("'system'"))
                && (txt.contains("f\"") || txt.contains("f'"))
            {
                push_llm(
                    out,
                    LlmAgentViolation::SystemPromptInjection,
                    node,
                    String::from(
                        "dynamic f-string in `(\"system\", f\"...\")` tuple risks prompt injection",
                    ),
                );
            }
        }
        _ => {}
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        walk_llm(ch, source, flags, out);
    }
}

/// Extracts `LangChain` 1.x & `LangGraph` 1.x CST findings (`LLM-*`) without I/O.
pub fn extract_llm_agent_rules(
    root: Node<'_>,
    source: &str,
    in_code: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let has_llm = [
        "langchain",
        "langgraph",
        "StateGraph",
        "PromptTemplate",
        "SystemMessage",
        "PythonREPLTool",
        "load_prompt",
        "ChatOpenAI",
        "ChatAnthropic",
        "allow_dangerous_",
        "secrets_from_env",
    ]
    .iter()
    .any(|m| source.contains(m));
    if !has_llm {
        return;
    }
    let has_end = !source.contains("StateGraph")
        || source.contains("END")
        || source.contains("\"__end__\"")
        || source.contains("'__end__'");
    let has_cond = source.contains("add_conditional_edges");
    let flags = u8::from(in_code) | (u8::from(has_end) << 1) | (u8::from(has_cond) << 2);
    walk_llm(root, source, flags, out);
}
