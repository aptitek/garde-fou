//! Strict `serde` TOML `[tools]` (`[tools.python]` & `[tools.typescript]`) section parser.

use gf_core::{PythonToolsConfig, ScannerPolicy, ToolsConfig, TypeScriptToolsConfig};
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
pub struct ToolsSection {
    python: Option<PythonToolsSection>,
    typescript: Option<TypeScriptToolsSection>,
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

pub fn merge_tools(raw: Option<ToolsSection>) -> Result<ToolsConfig, EngineError> {
    let base = ToolsConfig::defaults();
    let Some(section) = raw else {
        return Ok(base);
    };
    let python = if let Some(py) = section.python {
        PythonToolsConfig {
            ruff: parse_opt_tool_policy(py.ruff, base.python.ruff, "tools.python.ruff")?,
            mypy: parse_opt_tool_policy(py.mypy, base.python.mypy, "tools.python.mypy")?,
            pyright: parse_opt_tool_policy(
                py.pyright,
                base.python.pyright,
                "tools.python.pyright",
            )?,
        }
    } else {
        base.python
    };
    let typescript = if let Some(ts) = section.typescript {
        TypeScriptToolsConfig {
            oxlint: parse_opt_tool_policy(
                ts.oxlint,
                base.typescript.oxlint,
                "tools.typescript.oxlint",
            )?,
            biome: parse_opt_tool_policy(
                ts.biome,
                base.typescript.biome,
                "tools.typescript.biome",
            )?,
            tsc: parse_opt_tool_policy(ts.tsc, base.typescript.tsc, "tools.typescript.tsc")?,
        }
    } else {
        base.typescript
    };
    Ok(ToolsConfig { python, typescript })
}
