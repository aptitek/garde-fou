//! Unit tests verifying Python ecosystem violation metadata and Django migration path matching.

use gf_core::{
    DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, PyEcosystemViolation,
    is_django_migration_path,
};

struct Case {
    violation: PyEcosystemViolation,
    code: &'static str,
    needle: &'static str,
}

const fn case(violation: PyEcosystemViolation, code: &'static str, needle: &'static str) -> Case {
    Case {
        violation,
        code,
        needle,
    }
}

#[rustfmt::skip]
const CASES: &[Case] = &[
    case(PyEcosystemViolation::Django(DjangoViolation::RawSqlOrUnsafeHtml), "DJ-RAW-SQL-OR-UNSAFE-HTML", "format_html"),
    case(PyEcosystemViolation::Django(DjangoViolation::OrmNPlusOneAndUnbounded), "DJ-ORM-N-PLUS-ONE-AND-UNBOUNDED", "select_related"),
    case(PyEcosystemViolation::Django(DjangoViolation::ModelAndFormHygiene), "DJ-MODEL-AND-FORM-HYGIENE", "__str__"),
    case(PyEcosystemViolation::Django(DjangoViolation::SettingsAndAuthHardening), "DJ-SETTINGS-AND-AUTH-HARDENING", "DEBUG"),
    case(PyEcosystemViolation::NiceGui(NiceGuiViolation::BlockingEventLoop), "NICEGUI-BLOCKING-EVENT-LOOP", "run.io_bound"),
    case(PyEcosystemViolation::NiceGui(NiceGuiViolation::GlobalUiOrStateLeak), "NICEGUI-GLOBAL-UI-OR-STATE-LEAK", "@ui.page"),
    case(PyEcosystemViolation::NiceGui(NiceGuiViolation::WorkerContextViolation), "NICEGUI-WORKER-CONTEXT-VIOLATION", "app.storage.*"),
    case(PyEcosystemViolation::NiceGui(NiceGuiViolation::SecurityXssAndSecret), "NICEGUI-SECURITY-XSS-AND-SECRET", "storage_secret"),
    case(PyEcosystemViolation::MlDl(MlDlViolation::UnsafeModelDeserialization), "ML-UNSAFE-MODEL-DESERIALIZATION", "weights_only=True"),
    case(PyEcosystemViolation::MlDl(MlDlViolation::TorchTrainingAndVramBugs), "ML-TORCH-TRAINING-AND-VRAM-BUGS", "zero_grad"),
    case(PyEcosystemViolation::MlDl(MlDlViolation::TfRetracingAndEager), "ML-TF-RETRACING-AND-EAGER", "@tf.function"),
    case(PyEcosystemViolation::MlDl(MlDlViolation::DataLeakageBeforeSplit), "ML-DATA-LEAKAGE-BEFORE-SPLIT", "train_test_split"),
    case(PyEcosystemViolation::LlmAgent(LlmAgentViolation::RceAndCveVectors), "LLM-RCE-AND-CVE-VECTORS", "jinja2"),
    case(PyEcosystemViolation::LlmAgent(LlmAgentViolation::SystemPromptInjection), "LLM-SYSTEM-PROMPT-INJECTION", "system prompts"),
    case(PyEcosystemViolation::LlmAgent(LlmAgentViolation::LangGraphInfiniteLoopAndBudget), "LLM-LANGGRAPH-INFINITE-LOOP-AND-BUDGET", "recursion_limit"),
];

#[test]
fn py_ecosystem_violations_expose_exact_codes_remediations_and_display() {
    for c in CASES {
        assert_eq!(c.violation.code(), c.code);
        assert_eq!(format!("{}", c.violation), c.code);
        assert!(c.violation.remediation().contains(c.needle));
    }
}

#[test]
fn django_migration_path_classifier_handles_all_boundaries() {
    assert!(is_django_migration_path("blog/migrations/0001_initial.py"));
    assert!(is_django_migration_path("migrations/0002_auto.py"));
    assert!(!is_django_migration_path("blog/migrations/__init__.py"));
    assert!(!is_django_migration_path(
        "blog/migrations/0001_initial.sql"
    ));
    assert!(!is_django_migration_path("blog/models.py"));
}
