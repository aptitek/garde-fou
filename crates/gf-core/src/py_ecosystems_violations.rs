//! Closed algebraic sum types for Python ecosystem CST guardrails (`GF080`):
//! Django 5.2/6.0 (`DJ-*`), `NiceGUI` (`NICEGUI-*`), `PyTorch`/`Keras`/`Sklearn` (`ML-*`),
//! and `LangChain`/`LangGraph` (`LLM-*`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Django 5.2 LTS / 6.0+ CST violations (`DJ-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DjangoViolation {
    /// `.extra()`, dynamic `RawSQL`/`.raw()`/`cursor.execute()`, `mark_safe()`, or pre-formatted `format_html(f"...")`.
    RawSqlOrUnsafeHtml,
    /// `DJ001` (`null=True` on string field without `unique=True, blank=True`), `DJ008` (missing `__str__`), `DJ003` (`locals()`), `DJ007` (`fields = "__all__"`).
    ModelAndFormHygiene,
    /// ORM query inside loop (`N+1`) or unbounded `Model.objects.all()` returned in a view.
    OrmNPlusOneAndUnbounded,
    /// Production `settings.py` `DEBUG = True`, `ALLOWED_HOSTS = ["*"]`, `CORS_ALLOW_ALL_ORIGINS = True`, or unvaried `@cache_page`.
    SettingsAndAuthHardening,
}

impl DjangoViolation {
    /// Canonical `DJ-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "DJ-RAW-SQL-OR-UNSAFE-HTML",
            "DJ-MODEL-AND-FORM-HYGIENE",
            "DJ-ORM-N-PLUS-ONE-AND-UNBOUNDED",
            "DJ-SETTINGS-AND-AUTH-HARDENING",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 4] = [
            "Use parameterized ORM queries or `cursor.execute(sql, params)`, and pass separate args to `format_html(\"... {}\", val)` instead of f-strings or `mark_safe`.",
            "Avoid `null=True` on string fields (unless `unique=True, blank=True`), define `__str__` on concrete `Model` subclasses, list explicit form `fields`, and avoid `locals()` in `render()`.",
            "Preload relations with `select_related`/`prefetch_related` outside loops and paginate or slice `Model.objects.all()`.",
            "Disable `DEBUG`, pin `ALLOWED_HOSTS`, enforce secure cookies in prod settings, and guard authenticated `@cache_page` with `@vary_on_cookie` / `@vary_on_headers`.",
        ];
        REM[self as usize]
    }
}

/// `NiceGUI` reactive `asyncio` & multiprocessing CST violations (`NICEGUI-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum NiceGuiViolation {
    /// Blocking `time.sleep`, `requests.*`, `urlopen`, or `subprocess.run` inside `@ui.page` / `async def` / UI callback.
    BlockingEventLoop,
    /// Top-level `ui.*` elements alongside `@ui.page(...)` routes or `global` state mutation in UI handlers.
    GlobalUiOrStateLeak,
    /// Accessing `ui.*` or `app.storage.{user,tab,client,browser}` inside `run.io_bound`/`run.cpu_bound` worker or `@app.on_exception`.
    WorkerContextViolation,
    /// `ui.html(..., sanitize=False)`, interpolated `ui.run_javascript(f"...")`, or missing/hardcoded `storage_secret` in `ui.run(...)`.
    SecurityXssAndSecret,
}

impl NiceGuiViolation {
    /// Canonical `NICEGUI-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "NICEGUI-BLOCKING-EVENT-LOOP",
            "NICEGUI-GLOBAL-UI-OR-STATE-LEAK",
            "NICEGUI-WORKER-CONTEXT-VIOLATION",
            "NICEGUI-SECURITY-XSS-AND-SECRET",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 4] = [
            "Use `await asyncio.sleep(...)`, `httpx.AsyncClient`, or `await run.io_bound(...)` / `await run.cpu_bound(...)` to keep NiceGUI's event loop non-blocking.",
            "Encapsulate all `ui.*` elements inside `@ui.page(...)` functions and store per-user state in `app.storage.user` / `app.storage.tab` instead of module globals.",
            "Read `app.storage.*` in the async UI coroutine and pass pure serializable arguments to `run.io_bound`/`run.cpu_bound` workers; never call `ui.*` inside workers or `@app.on_exception`.",
            "Keep `ui.html(..., sanitize=True)`, serialize JS args with `json.dumps`, and load `storage_secret` from `os.environ` in `ui.run(...)`.",
        ];
        REM[self as usize]
    }
}

/// Deep Learning & Data Science (`PyTorch`, `TensorFlow`/`Keras 3`, `scikit-learn`) CST violations (`ML-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MlDlViolation {
    /// `torch.load(..., weights_only=False)`, `joblib.load`/`dill.load`/`cloudpickle.load`, `load_model(..., safe_mode=False)`, `enable_unsafe_deserialization()`, or legacy `.h5`/`.hdf5`.
    UnsafeModelDeserialization,
    /// Missing `zero_grad()` in training loop, autograd VRAM leak (`append(loss)` without `.item()`/`.detach()`), unguarded inference, or hardcoded `.cuda()`.
    TorchTrainingAndVramBugs,
    /// `@tf.function` declared inside a loop (retracing) or `tf.config.run_functions_eagerly(True)` in production code.
    TfRetracingAndEager,
    /// Preprocessor `.fit()` / `.fit_transform()` invoked before `train_test_split(...)` (data leakage).
    DataLeakageBeforeSplit,
}

impl MlDlViolation {
    /// Canonical `ML-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "ML-UNSAFE-MODEL-DESERIALIZATION",
            "ML-TORCH-TRAINING-AND-VRAM-BUGS",
            "ML-TF-RETRACING-AND-EAGER",
            "ML-DATA-LEAKAGE-BEFORE-SPLIT",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 4] = [
            "Use `torch.load(..., weights_only=True)` or `safetensors`, and Keras 3 native `.keras` format with `safe_mode=True`.",
            "Call `optimizer.zero_grad(set_to_none=True)` each step, detach scalars with `loss.item()`, wrap inference in `with torch.inference_mode():`, and use `.to(device)`.",
            "Define `@tf.function` at module/class scope outside loops and remove `tf.config.run_functions_eagerly(True)` from production code.",
            "Call `train_test_split(...)` before fitting scalers/vectorizers or wrap preprocessing inside `sklearn.pipeline.Pipeline`.",
        ];
        REM[self as usize]
    }
}

/// `LangChain` 1.x & `LangGraph` 1.x CST violations (`LLM-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LlmAgentViolation {
    /// `allow_dangerous_code=True`, `allow_dangerous_requests=True`, `PythonREPLTool`/`ShellTool`, `load_prompt`, `secrets_from_env=True`, or `template_format="jinja2"`.
    RceAndCveVectors,
    /// Dynamic f-string interpolation directly inside `SystemMessage(...)` or `("system", f"...")`.
    SystemPromptInjection,
    /// Compiled `StateGraph` missing an edge to `END`, cyclic graph invoked without `recursion_limit`, or LLM client missing `timeout`/`max_retries`.
    LangGraphInfiniteLoopAndBudget,
}

impl LlmAgentViolation {
    /// Canonical `LLM-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 3] = [
            "LLM-RCE-AND-CVE-VECTORS",
            "LLM-SYSTEM-PROMPT-INJECTION",
            "LLM-LANGGRAPH-INFINITE-LOOP-AND-BUDGET",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 3] = [
            "Remove `allow_dangerous_*`, REPL/Shell tools, `load_prompt`, `secrets_from_env=True`, and `jinja2` templates; use f-string-free `ChatPromptTemplate`.",
            "Keep system prompts static constants and pass dynamic user variables via `{var}` placeholders in `human` messages.",
            "Connect every compiled `StateGraph` to `END`, pass `config={\"recursion_limit\": N}` on cyclic invocations, and bound LLM clients with `timeout` and `max_retries`.",
        ];
        REM[self as usize]
    }
}

/// Unified closed sum type across all Python ecosystem analyzers (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PyEcosystemViolation {
    /// Django 5.2/6.0 violation (`DJ-*`).
    Django(DjangoViolation),
    /// `NiceGUI` violation (`NICEGUI-*`).
    NiceGui(NiceGuiViolation),
    /// Deep Learning / Data Science violation (`ML-*`).
    MlDl(MlDlViolation),
    /// `LangChain` / `LangGraph` violation (`LLM-*`).
    LlmAgent(LlmAgentViolation),
}

impl PyEcosystemViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Django(v) => v.code(),
            Self::NiceGui(v) => v.code(),
            Self::MlDl(v) => v.code(),
            Self::LlmAgent(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Django(v) => v.remediation(),
            Self::NiceGui(v) => v.remediation(),
            Self::MlDl(v) => v.remediation(),
            Self::LlmAgent(v) => v.remediation(),
        }
    }
}

/// Returns `true` when `path` is an auto-generated Django migration (`**/migrations/0*.py`).
#[must_use]
pub fn is_django_migration_path(path: &str) -> bool {
    let is_in_migrations = path.starts_with("migrations/") || path.contains("/migrations/");
    let base = path.rsplit('/').next().unwrap_or(path);
    is_in_migrations && base.starts_with('0') && base.strip_suffix(".py").is_some()
}

impl fmt::Display for PyEcosystemViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
