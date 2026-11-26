//! Strict `serde` TOML `[tools.go]`, `[tools.kotlin]`, and `[tools.csharp]` parsers.

use gf_core::{CSharpToolsConfig, GoToolsConfig, KotlinToolsConfig, ScannerPolicy};
use serde::Deserialize;

use crate::types::EngineError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoToolsSection {
    golangci_lint: Option<String>,
    govet: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KotlinToolsSection {
    detekt: Option<String>,
    ktlint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CSharpToolsSection {
    dotnet_build: Option<String>,
}

fn parse_policy(
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

pub fn merge_go_tools(
    raw: Option<GoToolsSection>,
    base: GoToolsConfig,
) -> Result<GoToolsConfig, EngineError> {
    let Some(g) = raw else { return Ok(base) };
    Ok(GoToolsConfig {
        golangci_lint: parse_policy(
            g.golangci_lint,
            base.golangci_lint,
            "tools.go.golangci_lint",
        )?,
        govet: parse_policy(g.govet, base.govet, "tools.go.govet")?,
    })
}

pub fn merge_kotlin_tools(
    raw: Option<KotlinToolsSection>,
    base: KotlinToolsConfig,
) -> Result<KotlinToolsConfig, EngineError> {
    let Some(k) = raw else { return Ok(base) };
    Ok(KotlinToolsConfig {
        detekt: parse_policy(k.detekt, base.detekt, "tools.kotlin.detekt")?,
        ktlint: parse_policy(k.ktlint, base.ktlint, "tools.kotlin.ktlint")?,
    })
}

pub fn merge_csharp_tools(
    raw: Option<CSharpToolsSection>,
    base: CSharpToolsConfig,
) -> Result<CSharpToolsConfig, EngineError> {
    let Some(c) = raw else { return Ok(base) };
    Ok(CSharpToolsConfig {
        dotnet_build: parse_policy(
            c.dotnet_build,
            base.dotnet_build,
            "tools.csharp.dotnet_build",
        )?,
    })
}
