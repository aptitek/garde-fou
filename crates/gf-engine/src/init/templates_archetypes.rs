//! Proportional starter templates for `Scripts`, `Infra`, `Knowledge` (Obsidian),
//! and `Academic` (Jupyter `.ipynb` & Quarto `.qmd`) archetypes.

use gf_core::{FrameworkId, LanguageId, ProjectContext, WorkspaceArchetype};

const POSIX_INSTALL_SH: &str = "\
#!/bin/sh
set -eu

main() {
    printf '%s\\n' \"ready\"
}

main \"$@\"
";

const STRICT_INSTALL_PS1: &str = "\
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-Main {
    Write-Output 'ready'
}

Invoke-Main
";

const PEP723_SCRIPT_PY: &str = "\
# /// script
# requires-python = \">=3.12\"
# dependencies = [
#   \"httpx>=0.27,<1\",
# ]
# ///

\"\"\"Standalone automation script managed by Garde-fou.\"\"\"


def main() -> None:
    \"\"\"Entry point for the standalone script.\"\"\"
    print(\"ready\")


if __name__ == \"__main__\":
    main()
";

const INFRA_COMPOSE_YAML: &str = "\
services:
  app:
    image: nginx:1.27-alpine
    restart: unless-stopped
    read_only: true
    security_opt:
      - no-new-privileges:true
    env_file:
      - .env.example
    ports:
      - \"127.0.0.1:8080:80\"
    deploy:
      resources:
        limits:
          cpus: \"0.50\"
          memory: 128M
    healthcheck:
      test: [\"CMD\", \"wget\", \"-q\", \"--spider\", \"http://127.0.0.1:80\"]
      interval: 30s
      timeout: 5s
      retries: 3
";

const INFRA_ENV_EXAMPLE: &str =
    "# Copy to .env and fill local values (never commit .env)\nAPP_ENV=development\n";

const INFRA_DOCKERFILE: &str = "\
# syntax=docker/dockerfile:1
FROM alpine:3.20
WORKDIR /app
RUN adduser -D -u 10001 appuser
USER appuser
CMD [\"sh\", \"-c\", \"echo ready\"]
";

const JUPYTER_STARTER_IPYNB: &str = "{\n\
  \"cells\": [\n\
    {\n\
      \"cell_type\": \"markdown\",\n\
      \"metadata\": {},\n\
      \"source\": [\n\
        \"# Notebook\\n\",\n\
        \"\\n\",\n\
        \"Reproducible notebook managed by Garde-fou.\\n\"\n\
      ]\n\
    },\n\
    {\n\
      \"cell_type\": \"code\",\n\
      \"execution_count\": 1,\n\
      \"metadata\": {},\n\
      \"outputs\": [],\n\
      \"source\": [\n\
        \"def square(value: int) -> int:\\n\",\n\
        \"    return value * value\\n\",\n\
        \"\\n\",\n\
        \"print(square(4))\\n\"\n\
      ]\n\
    }\n\
  ],\n\
  \"metadata\": {\n\
    \"kernelspec\": {\n\
      \"display_name\": \"Python 3\",\n\
      \"language\": \"python\",\n\
      \"name\": \"python3\"\n\
    },\n\
    \"language_info\": {\n\
      \"name\": \"python\"\n\
    }\n\
  },\n\
  \"nbformat\": 4,\n\
  \"nbformat_minor\": 5\n\
}\n";

const QUARTO_CONFIG_YML: &str = "\
project:
  title: \"academic-notes\"
  render:
    - index.qmd

format:
  html:
    toc: true
";

const QUARTO_INDEX_QMD: &str = "\
---
title: \"Course Notes\"
format: html
---

# Introduction {#sec-intro}

See @sec-intro for an overview.

```{python}
#| label: fig-demo
def compute(value: int) -> int:
    return value + 1

print(compute(1))
```
";

const OBSIDIAN_INDEX_MD: &str = "\
---
title: Vault Index
tags:
  - index
---

# Vault Index

Welcome to this Obsidian vault managed by Garde-fou.
";

const OBSIDIAN_APP_JSON: &str = "{\n  \"attachmentFolderPath\": \"assets\"\n}\n";

fn scripts_starter_files(ctx: &ProjectContext) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if ctx.languages.contains(&LanguageId::Shell) {
        files.push((String::from("install.sh"), String::from(POSIX_INSTALL_SH)));
    }
    if ctx.languages.contains(&LanguageId::PowerShell) {
        files.push((
            String::from("install.ps1"),
            String::from(STRICT_INSTALL_PS1),
        ));
    }
    if ctx.languages.contains(&LanguageId::Python) {
        files.push((String::from("script.py"), String::from(PEP723_SCRIPT_PY)));
    }
    files
}

fn academic_starter_files(ctx: &ProjectContext) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if ctx.frameworks.contains(&FrameworkId::Jupyter) {
        files.push((
            String::from("notebook.ipynb"),
            String::from(JUPYTER_STARTER_IPYNB),
        ));
    }
    if ctx.frameworks.contains(&FrameworkId::Quarto) {
        files.push((String::from("_quarto.yml"), String::from(QUARTO_CONFIG_YML)));
        files.push((String::from("index.qmd"), String::from(QUARTO_INDEX_QMD)));
    }
    files
}

/// Collects proportional starter files for `Minimal`, `Scripts`, `Infra`, `Knowledge`, and `Academic`.
pub fn archetype_starter_files(ctx: &ProjectContext) -> Vec<(String, String)> {
    match ctx.archetype {
        WorkspaceArchetype::Minimal | WorkspaceArchetype::Software => Vec::new(),
        WorkspaceArchetype::Scripts => scripts_starter_files(ctx),
        WorkspaceArchetype::Infra => vec![
            (
                String::from("compose.yaml"),
                String::from(INFRA_COMPOSE_YAML),
            ),
            (
                String::from(".env.example"),
                String::from(INFRA_ENV_EXAMPLE),
            ),
            (String::from("Dockerfile"), String::from(INFRA_DOCKERFILE)),
        ],
        WorkspaceArchetype::Knowledge => vec![
            (String::from("index.md"), String::from(OBSIDIAN_INDEX_MD)),
            (
                String::from(".obsidian/app.json"),
                String::from(OBSIDIAN_APP_JSON),
            ),
            (String::from("assets/.gitkeep"), String::new()),
        ],
        WorkspaceArchetype::Academic => academic_starter_files(ctx),
    }
}
