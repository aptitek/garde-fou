//! Unit tests verifying Python ecosystem violation metadata and Django migration path matching.

use gf_core::{
    DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, PyEcosystemViolation,
    is_django_migration_path,
};

#[rustfmt::skip]
const CASES: &[(PyEcosystemViolation, &str, &str)] = &[
    (PyEcosystemViolation::Django(DjangoViolation::RawSqlOrUnsafeHtml), "DJ-RAW-SQL-OR-UNSAFE-HTML", "format_html"),
    (PyEcosystemViolation::Django(DjangoViolation::OrmNPlusOneAndUnbounded), "DJ-ORM-N-PLUS-ONE-AND-UNBOUNDED", "select_related"),
    (PyEcosystemViolation::Django(DjangoViolation::ModelAndFormHygiene), "DJ-MODEL-AND-FORM-HYGIENE", "__str__"),
    (PyEcosystemViolation::Django(DjangoViolation::SettingsAndAuthHardening), "DJ-SETTINGS-AND-AUTH-HARDENING", "DEBUG"),
    (PyEcosystemViolation::NiceGui(NiceGuiViolation::BlockingEventLoop), "NICEGUI-BLOCKING-EVENT-LOOP", "run.io_bound"),
    (PyEcosystemViolation::NiceGui(NiceGuiViolation::GlobalUiOrStateLeak), "NICEGUI-GLOBAL-UI-OR-STATE-LEAK", "@ui.page"),
    (PyEcosystemViolation::NiceGui(NiceGuiViolation::WorkerContextViolation), "NICEGUI-WORKER-CONTEXT-VIOLATION", "app.storage.*"),
    (PyEcosystemViolation::NiceGui(NiceGuiViolation::SecurityXssAndSecret), "NICEGUI-SECURITY-XSS-AND-SECRET", "storage_secret"),
    (PyEcosystemViolation::MlDl(MlDlViolation::UnsafeModelDeserialization), "ML-UNSAFE-MODEL-DESERIALIZATION", "weights_only=True"),
    (PyEcosystemViolation::MlDl(MlDlViolation::TorchTrainingAndVramBugs), "ML-TORCH-TRAINING-AND-VRAM-BUGS", "zero_grad"),
    (PyEcosystemViolation::MlDl(MlDlViolation::TfRetracingAndEager), "ML-TF-RETRACING-AND-EAGER", "@tf.function"),
    (PyEcosystemViolation::MlDl(MlDlViolation::DataLeakageBeforeSplit), "ML-DATA-LEAKAGE-BEFORE-SPLIT", "train_test_split"),
    (PyEcosystemViolation::LlmAgent(LlmAgentViolation::RceAndCveVectors), "LLM-RCE-AND-CVE-VECTORS", "jinja2"),
    (PyEcosystemViolation::LlmAgent(LlmAgentViolation::SystemPromptInjection), "LLM-SYSTEM-PROMPT-INJECTION", "system prompts"),
    (PyEcosystemViolation::LlmAgent(LlmAgentViolation::LangGraphInfiniteLoopAndBudget), "LLM-LANGGRAPH-INFINITE-LOOP-AND-BUDGET", "recursion_limit"),
];

#[test]
fn py_ecosystem_violations_expose_exact_codes_remediations_and_display() {
    for &(v, expected_code, rem_needle) in CASES {
        assert_eq!(v.code(), expected_code);
        assert_eq!(format!("{v}"), expected_code);
        assert!(v.remediation().contains(rem_needle));
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
