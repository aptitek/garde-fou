//! Deep Learning & Data Science CST analyzer (`PyTorch`, `TensorFlow`/`Keras 3`, `scikit-learn`)
//! (`ML-*` under `GF080`).

use gf_core::{LangRuleFinding, MlDlViolation, Span};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

fn push_ml(out: &mut Vec<LangRuleFinding>, v: MlDlViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn check_keras_deser(node: Node<'_>, ftxt: &str, compact: &str, out: &mut Vec<LangRuleFinding>) {
    if ftxt.ends_with("load_model")
        && (compact.contains("safe_mode=False")
            || compact.contains(".h5\"")
            || compact.contains(".h5'")
            || compact.contains(".hdf5\"")
            || compact.contains(".hdf5'"))
    {
        push_ml(
            out,
            MlDlViolation::UnsafeModelDeserialization,
            node,
            String::from(
                "unsafe Keras `load_model`: forbid `safe_mode=False` and legacy `.h5`/`.hdf5` (use `.keras` with `safe_mode=True`)",
            ),
        );
    }
    if ftxt.ends_with("enable_unsafe_deserialization") {
        push_ml(
            out,
            MlDlViolation::UnsafeModelDeserialization,
            node,
            String::from("`enable_unsafe_deserialization()` disables Keras 3 security sandbox"),
        );
    }
}

fn check_ml_deser(node: Node<'_>, ftxt: &str, compact: &str, out: &mut Vec<LangRuleFinding>) {
    if ftxt == "torch.load" && compact.contains("weights_only=False") {
        push_ml(
            out,
            MlDlViolation::UnsafeModelDeserialization,
            node,
            String::from(
                "`torch.load(..., weights_only=False)` enables arbitrary pickle RCE; use `weights_only=True` or `safetensors`",
            ),
        );
    }
    check_keras_deser(node, ftxt, compact, out);
}

fn check_ml_call(node: Node<'_>, source: &str, in_code: bool, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, source).trim();
    let args_txt = node
        .child_by_field_name("arguments")
        .map_or("", |a| node_text(a, source));
    let compact: String = args_txt.chars().filter(|c| !c.is_whitespace()).collect();
    check_ml_deser(node, ftxt, &compact, out);
    if matches!(
        ftxt,
        "joblib.load" | "dill.load" | "dill.loads" | "cloudpickle.load" | "cloudpickle.loads"
    ) && in_code
    {
        let msg = format!(
            "`{ftxt}(...)` deserializes untrusted pickle streams; prefer `safetensors` / `onnx`"
        );
        push_ml(out, MlDlViolation::UnsafeModelDeserialization, node, msg);
    }
    if ftxt.rsplit('.').next() == Some("cuda")
        || (ftxt == "torch.device" && compact.contains("\"cuda\""))
    {
        push_ml(
            out,
            MlDlViolation::TorchTrainingAndVramBugs,
            node,
            String::from("avoid hardcoded `.cuda()` / `torch.device(\"cuda\")`; use `.to(device)`"),
        );
    }
    if in_code && ftxt.ends_with("run_functions_eagerly") && compact.contains("True") {
        push_ml(
            out,
            MlDlViolation::TfRetracingAndEager,
            node,
            String::from("remove `tf.config.run_functions_eagerly(True)` from production code"),
        );
    }
}

fn check_loop_vram_node(n: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let ntxt = node_text(n, source);
    if ntxt.contains(".item()") || ntxt.contains(".detach()") {
        return;
    }
    let is_append_leak = n.kind() == "call" && ntxt.contains(".append(") && ntxt.contains("loss");
    let is_aug_leak = n.kind() == "augmented_assignment" && ntxt.contains("+= loss");
    if is_append_leak || is_aug_leak {
        push_ml(
            out,
            MlDlViolation::TorchTrainingAndVramBugs,
            n,
            String::from(
                "retaining raw `loss` tensor in loop leaks autograd graph VRAM; use `loss.item()` or `loss.detach()`",
            ),
        );
    }
}

fn check_torch_training_loop(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let txt = node_text(node, source);
    if !txt.contains(".backward()") {
        return;
    }
    if txt.contains(".step()") && !txt.contains(".zero_grad(") {
        push_ml(
            out,
            MlDlViolation::TorchTrainingAndVramBugs,
            node,
            String::from(
                "PyTorch training loop calls `.backward()` and `.step()` without `.zero_grad(...)`",
            ),
        );
    }
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        check_loop_vram_node(n, source, out);
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
}

fn walk_ml(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let in_code = (flags & 1) != 0;
    let in_loop = (flags & 2) != 0;
    match node.kind() {
        "call" => check_ml_call(node, source, in_code, out),
        "for_statement" | "while_statement" => check_torch_training_loop(node, source, out),
        "decorated_definition" if in_loop && node_text(node, source).contains("@tf.function") => {
            push_ml(
                out,
                MlDlViolation::TfRetracingAndEager,
                node,
                String::from(
                    "`@tf.function` declared inside a loop causes expensive graph retracing",
                ),
            );
        }
        _ => {}
    }
    let next_flags = if matches!(node.kind(), "for_statement" | "while_statement") {
        flags | 2
    } else {
        flags
    };
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        walk_ml(ch, source, next_flags, out);
    }
}

/// Detects preprocessor `.fit()` / `.fit_transform()` invoked before `train_test_split(...)`
/// across either a single Python file or a sequence of notebook cells (`ML-DATA-LEAKAGE-BEFORE-SPLIT`).
fn check_data_leakage_in_source(source: &str, out: &mut Vec<LangRuleFinding>) {
    let mut first_fit: Option<u32> = None;
    let mut first_split: Option<u32> = None;
    for (idx, raw) in source.lines().enumerate() {
        let trimmed = raw.split('#').next().unwrap_or(raw).trim();
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        if first_fit.is_none()
            && (trimmed.contains(".fit_transform(") || trimmed.contains("scaler.fit("))
            && !trimmed.contains("Pipeline(")
        {
            first_fit = Some(line_no);
        }
        if first_split.is_none() && trimmed.contains("train_test_split(") {
            first_split = Some(line_no);
        }
    }
    if let (Some(fit_line), Some(split_line)) = (first_fit, first_split)
        && fit_line < split_line
    {
        let span = Span::new(fit_line, fit_line).unwrap_or(Span::FIRST);
        out.push(LangRuleFinding::new(
            MlDlViolation::DataLeakageBeforeSplit,
            span,
            String::from(
                "preprocessor `.fit()`/`.fit_transform()` called before `train_test_split(...)` causes test-set data leakage",
            ),
        ));
    }
}

/// Extracts `PyTorch`, `TensorFlow`/`Keras 3`, and `scikit-learn` CST findings (`ML-*`) without I/O.
pub fn extract_ml_dl_rules(
    root: Node<'_>,
    source: &str,
    in_code: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    walk_ml(root, source, u8::from(in_code), out);
    check_data_leakage_in_source(source, out);
}
