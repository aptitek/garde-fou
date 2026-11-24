//! Strict `serde` TOML `[tools]` (`python`, `typescript`, `shell`, `powershell`, `docker`) parser.

use gf_core::{
    DockerToolsConfig, PowerShellToolsConfig, PythonToolsConfig, ScannerPolicy, ShellToolsConfig,
    ToolsConfig, TypeScriptToolsConfig,
};
use serde::Deserialize;

use crate::types::EngineError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonToolsSection {
    ruff: Option<String>,
    mypy: Option<String>,
    pyright: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeScriptToolsSection {
    oxlint: Option<String>,
    biome: Option<String>,
    tsc: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellToolsSection {
    shellcheck: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerShellToolsSection {
    psscriptanalyzer: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DockerToolsSection {
    hadolint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsSection {
    python: Option<PythonToolsSection>,
    typescript: Option<TypeScriptToolsSection>,
    shell: Option<ShellToolsSection>,
    powershell: Option<PowerShellToolsSection>,
    docker: Option<DockerToolsSection>,
}

fn parse_opt_tool_policy(
    raw: Option<String>,
    default: ScannerPolicy,
    field: &str,
) -> Result<ScannerPolicy, EngineError> {
    raw.map_or(Ok(default), |val| {
        ScannerPolicy::from_config_str(&val).ok_or_else(|| {
            EngineError::InvalidConfig(format!(
                "invalid `{field}` policy `{val}` (expected `auto`, `required`, or `off`)"
            ))
        })
    })
}

fn merge_py_tools(
    raw: Option<PythonToolsSection>,
    base: PythonToolsConfig,
) -> Result<PythonToolsConfig, EngineError> {
    let Some(py) = raw else {
        return Ok(base);
    };
    Ok(PythonToolsConfig {
        ruff: parse_opt_tool_policy(py.ruff, base.ruff, "tools.python.ruff")?,
        mypy: parse_opt_tool_policy(py.mypy, base.mypy, "tools.python.mypy")?,
        pyright: parse_opt_tool_policy(py.pyright, base.pyright, "tools.python.pyright")?,
    })
}

fn merge_ts_tools(
    raw: Option<TypeScriptToolsSection>,
    base: TypeScriptToolsConfig,
) -> Result<TypeScriptToolsConfig, EngineError> {
    let Some(ts) = raw else {
        return Ok(base);
    };
    Ok(TypeScriptToolsConfig {
        oxlint: parse_opt_tool_policy(ts.oxlint, base.oxlint, "tools.typescript.oxlint")?,
        biome: parse_opt_tool_policy(ts.biome, base.biome, "tools.typescript.biome")?,
        tsc: parse_opt_tool_policy(ts.tsc, base.tsc, "tools.typescript.tsc")?,
    })
}

pub fn merge_tools(raw: Option<ToolsSection>) -> Result<ToolsConfig, EngineError> {
    let base = ToolsConfig::defaults();
    let Some(section) = raw else {
        return Ok(base);
    };
    let shell = if let Some(sh) = section.shell {
        ShellToolsConfig {
            shellcheck: parse_opt_tool_policy(
                sh.shellcheck,
                base.shell.shellcheck,
                "tools.shell.shellcheck",
            )?,
        }
    } else {
        base.shell
    };
    let powershell = if let Some(ps) = section.powershell {
        PowerShellToolsConfig {
            psscriptanalyzer: parse_opt_tool_policy(
                ps.psscriptanalyzer,
                base.powershell.psscriptanalyzer,
                "tools.powershell.psscriptanalyzer",
            )?,
        }
    } else {
        base.powershell
    };
    let docker = if let Some(dk) = section.docker {
        DockerToolsConfig {
            hadolint: parse_opt_tool_policy(
                dk.hadolint,
                base.docker.hadolint,
                "tools.docker.hadolint",
            )?,
        }
    } else {
        base.docker
    };
    Ok(ToolsConfig {
        python: merge_py_tools(section.python, base.python)?,
        typescript: merge_ts_tools(section.typescript, base.typescript)?,
        shell,
        powershell,
        docker,
    })
}
