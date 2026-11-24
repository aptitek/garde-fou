//! Modern `uv` + `ruff` + `mypy --strict` + `pytest` Python scaffolding templates.

use gf_core::{FrameworkId, ProjectContext};

fn py_dependencies(ctx: &ProjectContext) -> &'static str {
    let has_fastapi = ctx.frameworks.contains(&FrameworkId::FastApi);
    let has_django = ctx.frameworks.contains(&FrameworkId::Django);
    match (has_fastapi, has_django) {
        (true, true) => "[\"fastapi>=0.115.0\", \"django>=5.1.0\"]",
        (true, false) => "[\"fastapi>=0.115.0\"]",
        (false, true) => "[\"django>=5.1.0\"]",
        (false, false) => "[]",
    }
}

fn pyproject_toml(project_kebab: &str, ctx: &ProjectContext) -> String {
    let deps = py_dependencies(ctx);
    format!(
        "[project]\n\
         name = \"{project_kebab}\"\n\
         version = \"0.1.0\"\n\
         requires-python = \">=3.12\"\n\
         dependencies = {deps}\n\
         \n\
         [build-system]\n\
         requires = [\"hatchling\"]\n\
         build-backend = \"hatchling.build\"\n\
         \n\
         [tool.ruff]\n\
         line-length = 100\n\
         target-version = \"py312\"\n\
         \n\
         [tool.ruff.lint]\n\
         select = [\"E\", \"F\", \"I\", \"B\", \"UP\", \"SIM\", \"ANN\", \"S\", \"T20\"]\n\
         \n\
         [tool.mypy]\n\
         python_version = \"3.12\"\n\
         mypy_path = \"src\"\n\
         strict = true\n\
         \n\
         [tool.pytest.ini_options]\n\
         testpaths = [\"tests\"]\n\
         pythonpath = [\"src\"]\n\
         addopts = [\"--strict-markers\", \"--strict-config\"]\n"
    )
}

fn fastapi_main_py(project_kebab: &str, pkg_snake: &str) -> String {
    format!(
        "\"\"\"ASGI entrypoint for `{project_kebab}`.\"\"\"\n\
         \n\
         from {pkg_snake}.core import build_health_status\n\
         \n\
         \n\
         def healthcheck() -> dict[str, str]:\n\
         \x20   return build_health_status(\"{project_kebab}\")\n"
    )
}

fn django_views_py(project_kebab: &str, pkg_snake: &str) -> String {
    format!(
        "\"\"\"Django HTTP view handlers for `{project_kebab}`.\"\"\"\n\
         \n\
         from {pkg_snake}.core import build_health_status\n\
         \n\
         \n\
         def status_view() -> dict[str, str]:\n\
         \x20   return build_health_status(\"{project_kebab}\")\n"
    )
}

fn base_python_package_files(project_kebab: &str, pkg_snake: &str) -> [(String, String); 3] {
    let init_py = format!(
        "\"\"\"Top-level package for `{pkg_snake}`.\"\"\"\n\
         \n\
         from .core import build_health_status\n\
         \n\
         __all__ = [\"build_health_status\"]\n"
    );
    let core_py = String::from(
        "\"\"\"Pure domain logic.\"\"\"\n\
         \n\
         \n\
         def build_health_status(service_name: str) -> dict[str, str]:\n\
         \x20   clean = service_name.strip() or \"service\"\n\
         \x20   return {\"service\": clean, \"status\": \"ok\"}\n",
    );
    let test_py = format!(
        "\"\"\"Unit tests for `{pkg_snake}`.\"\"\"\n\
         \n\
         from {pkg_snake}.core import build_health_status\n\
         \n\
         \n\
         def test_build_health_status() -> None:\n\
         \x20   payload = build_health_status(\"{project_kebab}\")\n\
         \x20   assert payload == {{\"service\": \"{project_kebab}\", \"status\": \"ok\"}}\n"
    );
    [
        (format!("src/{pkg_snake}/__init__.py"), init_py),
        (format!("src/{pkg_snake}/core.py"), core_py),
        (String::from("tests/test_core.py"), test_py),
    ]
}

/// Generates non-destructive Python starter files `(relative_path, contents)`.
pub fn python_files(
    project_kebab: &str,
    pkg_snake: &str,
    ctx: &ProjectContext,
) -> Vec<(String, String)> {
    let mut files = vec![(
        String::from("pyproject.toml"),
        pyproject_toml(project_kebab, ctx),
    )];
    files.extend(base_python_package_files(project_kebab, pkg_snake));

    if ctx.frameworks.contains(&FrameworkId::FastApi) {
        files.push((
            String::from("app/main.py"),
            fastapi_main_py(project_kebab, pkg_snake),
        ));
    }
    if ctx.frameworks.contains(&FrameworkId::Django) {
        files.push((
            format!("src/{pkg_snake}/views.py"),
            django_views_py(project_kebab, pkg_snake),
        ));
    }
    files
}
