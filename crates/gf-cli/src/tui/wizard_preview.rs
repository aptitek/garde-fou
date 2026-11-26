//! Pure non-destructive file-tree preview builder for Step 4 (`Confirm`) of `garde init`.

use gf_core::{FeatureId, LanguageId, WorkspaceArchetype};
use gf_engine::InitRequest;

/// Computes a pure preview list of workspace files that `req` will scaffold on confirmation.
#[must_use]
pub fn preview_init_files(req: &InitRequest) -> Vec<String> {
    let mut files = vec![
        String::from(".gitattributes"),
        String::from(".gitignore"),
        String::from("garde-fou.toml"),
        String::from("AGENTS.md"),
        String::from(".agents/skills/garde-fou/SKILL.md"),
        String::from("garde.lock"),
    ];
    let arch = req.archetype.unwrap_or(WorkspaceArchetype::Minimal);
    append_archetype_preview(arch, req, &mut files);
    append_feature_preview(req, &mut files);
    files
}

fn append_archetype_preview(arch: WorkspaceArchetype, req: &InitRequest, out: &mut Vec<String>) {
    const LANG_PREVIEWS: &[(LanguageId, &str)] = &[
        (LanguageId::Rust, "Cargo.toml & src/lib.rs"),
        (
            LanguageId::TypeScript,
            "package.json, tsconfig.json & src/index.ts",
        ),
        (LanguageId::Python, "pyproject.toml & src/__init__.py"),
    ];
    match arch {
        WorkspaceArchetype::Software => {
            for (lang, label) in LANG_PREVIEWS {
                if req.languages.contains(lang) {
                    out.push(String::from(*label));
                }
            }
        }
        WorkspaceArchetype::Infra => out.push(String::from("compose.yaml")),
        WorkspaceArchetype::Knowledge => out.push(String::from("notes/index.md")),
        WorkspaceArchetype::Academic => out.push(String::from("_quarto.yml & courses/intro.qmd")),
        WorkspaceArchetype::Scripts => out.push(String::from("scripts/run.sh")),
        WorkspaceArchetype::Minimal => {}
    }
}

fn append_feature_preview(req: &InitRequest, out: &mut Vec<String>) {
    const FEAT_PREVIEWS: &[(FeatureId, &str)] = &[
        (FeatureId::Docker, "Dockerfile & .dockerignore"),
        (FeatureId::GitHooks, "lefthook.yml"),
        (FeatureId::TaskRunner, "Taskfile.yml & mise.toml"),
        (FeatureId::CiGithub, ".github/workflows/ci.yml"),
        (FeatureId::I18n, "locales/en.json & locales/fr.json"),
        (FeatureId::Content, "content/index.md"),
    ];
    for (feat, label) in FEAT_PREVIEWS {
        if req.features.contains(feat) {
            out.push(String::from(*label));
        }
    }
    if req.features.contains(&FeatureId::Ui)
        || req.features.contains(&FeatureId::Material3Expressive)
    {
        out.push(String::from("styles/tokens.css & styles/app.css"));
    }
}
