//! Infrastructure & governance templates (`garde-fou.toml`, `AGENTS.md`, `.gitattributes`,
//! hardened multi-stage `Dockerfile`, `lefthook.yml`, `Taskfile.yml`, `.github/workflows/garde-fou.yml`).

use gf_core::{FeatureId, LanguageId, ProjectContext};

/// Default cross-OS line-ending policy written to `.gitattributes` by `garde init`.
pub const GITATTRIBUTES_TEMPLATE: &str = "* text=auto eol=lf\n\
*.ps1 text eol=crlf\n\
*.bat text eol=crlf\n\
*.cmd text eol=crlf\n";

const LEFTHOOK_TEMPLATE: &str = "pre-commit:\n\
  parallel: true\n\
  commands:\n\
    garde:\n\
      run: garde check --staged --format agent\n\
\n\
commit-msg:\n\
  commands:\n\
    conventional:\n\
      run: garde commit-msg {1}\n\
\n\
pre-push:\n\
  parallel: true\n\
  commands:\n\
    garde-full:\n\
      run: garde check --no-cache\n";

const TASKFILE_TEMPLATE: &str = "version: \"3\"\n\
\n\
tasks:\n\
  check:\n\
    desc: Run full Garde-fou static analysis and security gates\n\
    cmds:\n\
      - garde check\n\
  check:staged:\n\
    desc: Run fast incremental check on staged Git changes\n\
    cmds:\n\
      - garde check --staged --format agent\n";

const GITHUB_WORKFLOW_TEMPLATE: &str = "name: garde-fou\n\
on:\n\
  push:\n\
    branches: [main]\n\
  pull_request:\n\
\n\
permissions:\n\
  contents: read\n\
\n\
jobs:\n\
  quality:\n\
    runs-on: ubuntu-latest\n\
    steps:\n\
      - uses: actions/checkout@v4\n\
      - name: Install garde (SHA-256 verified)\n\
        run: |\n\
          set -euo pipefail\n\
          BASE_URL=\"https://github.com/aptitek/garde-fou/releases/latest/download\"\n\
          ARCHIVE=\"garde-x86_64-unknown-linux-gnu.tar.gz\"\n\
          curl -fsSLO \"${BASE_URL}/${ARCHIVE}\"\n\
          curl -fsSLO \"${BASE_URL}/${ARCHIVE}.sha256\"\n\
          sha256sum -c \"${ARCHIVE}.sha256\"\n\
          mkdir -p \"$HOME/.local/bin\"\n\
          tar -xzf \"${ARCHIVE}\" -C \"$HOME/.local/bin\" garde\n\
          rm -f \"${ARCHIVE}\" \"${ARCHIVE}.sha256\"\n\
          echo \"$HOME/.local/bin\" >> \"$GITHUB_PATH\"\n\
      - name: Run Garde-fou\n\
        run: garde check --no-cache\n";

fn format_quoted_list<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let quoted: Vec<String> = items.map(|s| format!("\"{s}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

/// Normalizes a raw directory or CLI name into `(kebab-case, snake_case)` identifiers.
pub fn normalize_project_names(raw_name: &str) -> (String, String) {
    let kebab = sanitize_slug(raw_name, '-');
    let snake = sanitize_slug(&kebab, '_');
    (kebab, snake)
}

fn sanitize_slug(raw: &str, sep: char) -> String {
    let mut out = String::new();
    let mut prev_sep = true;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if !prev_sep {
            out.push(sep);
            prev_sep = true;
        }
    }
    let trimmed = out.trim_matches(sep);
    if trimmed.is_empty() {
        String::from("app")
    } else if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("app{sep}{trimmed}")
    } else {
        String::from(trimmed)
    }
}

const LIMITS_TOML_BLOCK: &str = "[limits]\n\
max_file_lines = 250\n\
max_function_lines = 50\n\
max_parameters = 4\n\
max_cyclomatic_complexity = 10\n\
max_cognitive_complexity = 10\n\
max_nesting_depth = 4\n\
max_imports_per_file = 15\n\
max_methods_per_type = 10\n\
min_clone_lines = 15\n\
min_clone_nodes = 40\n";

/// Renders a strict `garde-fou.toml` tailored to `project_name` and `ctx`.
pub fn render_garde_fou_toml(project_name: &str, ctx: &ProjectContext) -> String {
    let langs = format_quoted_list(ctx.languages.iter().map(|l| l.as_str()));
    let feats = format_quoted_list(ctx.features.iter().map(|f| f.as_str()));
    let fws = format_quoted_list(ctx.frameworks.iter().map(|fw| fw.as_str()));

    let py_tools_block = if ctx.languages.contains(&LanguageId::Python) {
        "[tools.python]\nruff = \"auto\"\nmypy = \"auto\"\npyright = \"off\"\n\n"
    } else {
        ""
    };

    format!(
        "# Garde-fou configuration\n\
         schema_version = 1\n\n\
         [project]\n\
         name = \"{project_name}\"\n\
         languages = {langs}\n\
         features = {feats}\n\
         frameworks = {fws}\n\n\
         {LIMITS_TOML_BLOCK}\n\
         [integrity]\n\
         require_lock = true\n\n\
         [agents]\n\
         sync_policy = \"auto\"\n\
         remote_skills = true\n\n\
         {py_tools_block}\
         [scanners]\n\
         secrets = \"auto\"\n\
         vulns = \"auto\"\n\n\
         [paths]\n\
         ignore_paths = []\n\
         entrypoints = []\n\n\
         [cache]\n\
         enabled = true\n\
         directory = \".garde-fou-cache\"\n"
    )
}

const PYTHON_DOCKERFILE: &str = "# syntax=docker/dockerfile:1\n\
FROM python:3.13-slim AS builder\n\
WORKDIR /build\n\
COPY pyproject.toml ./\n\
COPY src ./src\n\
RUN pip install --no-cache-dir --no-deps .\n\
\n\
FROM python:3.13-slim AS runtime\n\
WORKDIR /app\n\
RUN useradd --system --uid 10001 --no-create-home appuser\n\
COPY --from=builder /usr/local /usr/local\n\
COPY src ./src\n\
USER appuser\n\
CMD [\"python3\", \"-c\", \"print('ready')\"]\n";

const NODE_DOCKERFILE: &str = "# syntax=docker/dockerfile:1\n\
FROM node:22-slim AS builder\n\
WORKDIR /build\n\
COPY package.json tsconfig.json ./\n\
COPY src ./src\n\
\n\
FROM node:22-slim AS runtime\n\
WORKDIR /app\n\
RUN useradd --system --uid 10001 --no-create-home appuser\n\
COPY --from=builder /build/package.json /app/package.json\n\
COPY --from=builder /build/src /app/src\n\
USER appuser\n\
CMD [\"node\", \"--version\"]\n";

fn hardened_dockerfile(project_kebab: &str, ctx: &ProjectContext) -> String {
    if ctx.languages.contains(&LanguageId::Rust) {
        return format!(
            "# syntax=docker/dockerfile:1\n\
             FROM rust:1.88-slim AS builder\n\
             WORKDIR /build\n\
             COPY Cargo.toml ./\n\
             COPY src ./src\n\
             RUN cargo test --release\n\
             \n\
             FROM debian:bookworm-slim AS runtime\n\
             WORKDIR /app\n\
             RUN useradd --system --uid 10001 --no-create-home appuser\n\
             COPY --from=builder /build/Cargo.toml /app/{project_kebab}.toml\n\
             USER appuser\n\
             CMD [\"cat\", \"/app/{project_kebab}.toml\"]\n"
        );
    }
    if ctx.languages.contains(&LanguageId::Python) {
        return String::from(PYTHON_DOCKERFILE);
    }
    String::from(NODE_DOCKERFILE)
}

/// Generates optional feature infrastructure files `(relative_path, contents)`.
pub fn infra_feature_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if ctx.features.contains(&FeatureId::Docker) {
        files.push((
            String::from("Dockerfile"),
            hardened_dockerfile(project_kebab, ctx),
        ));
    }
    if ctx.features.contains(&FeatureId::GitHooks) {
        files.push((
            String::from("lefthook.yml"),
            String::from(LEFTHOOK_TEMPLATE),
        ));
    }
    if ctx.features.contains(&FeatureId::TaskRunner) {
        files.push((
            String::from("Taskfile.yml"),
            String::from(TASKFILE_TEMPLATE),
        ));
    }
    if ctx.features.contains(&FeatureId::CiGithub) {
        files.push((
            String::from(".github/workflows/garde-fou.yml"),
            String::from(GITHUB_WORKFLOW_TEMPLATE),
        ));
    }
    files
}
