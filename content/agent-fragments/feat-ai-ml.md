## AI / ML / Deep Learning & LLM Agent Standards (`GF080`–`GF081`)

### Model Serialization Safety, VRAM Hygiene, Anti-Leakage & Bounded Graphs

- Use `torch.load(..., weights_only=True)` or `safetensors` and Keras 3 `.keras` with `safe_mode=True`; forbid `weights_only=False`, `safe_mode=False`, `enable_unsafe_deserialization()`, legacy `.h5`/`.hdf5`, and untrusted `joblib`/`dill`/`cloudpickle` loads (`ML-UNSAFE-MODEL-DESERIALIZATION`).
- In PyTorch training/inference: call `optimizer.zero_grad(set_to_none=True)`, detach logged losses with `.item()`/`.detach()`, wrap inference in `with torch.inference_mode():`, and use `.to(device)` instead of hardcoded `.cuda()` (`ML-TORCH-TRAINING-AND-VRAM-BUGS`).
- Define `@tf.function` outside loops and forbid `run_functions_eagerly(True)` in production (`ML-TF-RETRACING-AND-EAGER`); always run `train_test_split(...)` before fitting preprocessors or use `sklearn.pipeline.Pipeline` (`ML-DATA-LEAKAGE-BEFORE-SPLIT`).
- In LangChain / LangGraph: forbid `allow_dangerous_*`, `PythonREPLTool`/`ShellTool`, `load_prompt`, `secrets_from_env=True`, `jinja2` templates (`LLM-RCE-AND-CVE-VECTORS`), and f-string system prompts (`LLM-SYSTEM-PROMPT-INJECTION`); connect every compiled `StateGraph` to `END` and enforce `timeout`/`max_retries` + `recursion_limit` (`LLM-LANGGRAPH-INFINITE-LOOP-AND-BUDGET`).
