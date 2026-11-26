//! Strict `serde` TOML `[tools]` (`python`, `typescript`, `shell`, `powershell`, `docker`,
//! `yaml`, `css`, `markdown`, `toml`, `xml`, `sql`) parser.

use gf_core::{
    CssToolsConfig, DockerToolsConfig, MarkdownToolsConfig, PowerShellToolsConfig,
    PythonToolsConfig, ScannerPolicy, ShellToolsConfig, SqlToolsConfig, TomlToolsConfig,
    ToolsConfig, TypeScriptToolsConfig, WebToolsConfig, XmlToolsConfig, YamlToolsConfig,
};
use serde::Deserialize;

use crate::config_tools_compiled::{
    CSharpToolsSection, GoToolsSection, KotlinToolsSection, merge_csharp_tools, merge_go_tools,
    merge_kotlin_tools,
};
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
struct YamlToolsSection {
    yamllint: Option<String>,
    dclint: Option<String>,
    actionlint: Option<String>,
    zizmor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CssToolsSection {
    stylelint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkdownToolsSection {
    rumdl: Option<String>,
    markdownlint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlToolsSection {
    taplo: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct XmlToolsSection {
    xmllint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SqlToolsSection {
    squawk: Option<String>,
    sqruff: Option<String>,
    sqlfluff: Option<String>,
    zpa: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WebToolsSection {
    astro_check: Option<String>,
    html_validate: Option<String>,
    wrangler_types: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsSection {
    python: Option<PythonToolsSection>,
    typescript: Option<TypeScriptToolsSection>,
    shell: Option<ShellToolsSection>,
    powershell: Option<PowerShellToolsSection>,
    docker: Option<DockerToolsSection>,
    yaml: Option<YamlToolsSection>,
    css: Option<CssToolsSection>,
    markdown: Option<MarkdownToolsSection>,
    toml: Option<TomlToolsSection>,
    xml: Option<XmlToolsSection>,
    sql: Option<SqlToolsSection>,
    web: Option<WebToolsSection>,
    go: Option<GoToolsSection>,
    kotlin: Option<KotlinToolsSection>,
    csharp: Option<CSharpToolsSection>,
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
    let Some(py) = raw else { return Ok(base) };
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
    let Some(ts) = raw else { return Ok(base) };
    Ok(TypeScriptToolsConfig {
        oxlint: parse_opt_tool_policy(ts.oxlint, base.oxlint, "tools.typescript.oxlint")?,
        biome: parse_opt_tool_policy(ts.biome, base.biome, "tools.typescript.biome")?,
        tsc: parse_opt_tool_policy(ts.tsc, base.tsc, "tools.typescript.tsc")?,
    })
}

fn merge_yaml_tools(
    raw: Option<YamlToolsSection>,
    base: YamlToolsConfig,
) -> Result<YamlToolsConfig, EngineError> {
    let Some(yml) = raw else { return Ok(base) };
    Ok(YamlToolsConfig {
        yamllint: parse_opt_tool_policy(yml.yamllint, base.yamllint, "tools.yaml.yamllint")?,
        dclint: parse_opt_tool_policy(yml.dclint, base.dclint, "tools.yaml.dclint")?,
        actionlint: parse_opt_tool_policy(
            yml.actionlint,
            base.actionlint,
            "tools.yaml.actionlint",
        )?,
        zizmor: parse_opt_tool_policy(yml.zizmor, base.zizmor, "tools.yaml.zizmor")?,
    })
}

fn merge_md_tools(
    raw: Option<MarkdownToolsSection>,
    base: MarkdownToolsConfig,
) -> Result<MarkdownToolsConfig, EngineError> {
    let Some(md) = raw else { return Ok(base) };
    Ok(MarkdownToolsConfig {
        rumdl: parse_opt_tool_policy(md.rumdl, base.rumdl, "tools.markdown.rumdl")?,
        markdownlint: parse_opt_tool_policy(
            md.markdownlint,
            base.markdownlint,
            "tools.markdown.markdownlint",
        )?,
    })
}

fn merge_sql_tools(
    raw: Option<SqlToolsSection>,
    base: SqlToolsConfig,
) -> Result<SqlToolsConfig, EngineError> {
    let Some(sq) = raw else { return Ok(base) };
    Ok(SqlToolsConfig {
        squawk: parse_opt_tool_policy(sq.squawk, base.squawk, "tools.sql.squawk")?,
        sqruff: parse_opt_tool_policy(sq.sqruff, base.sqruff, "tools.sql.sqruff")?,
        sqlfluff: parse_opt_tool_policy(sq.sqlfluff, base.sqlfluff, "tools.sql.sqlfluff")?,
        zpa: parse_opt_tool_policy(sq.zpa, base.zpa, "tools.sql.zpa")?,
    })
}

#[rustfmt::skip]
fn merge_web_tools(raw: Option<WebToolsSection>, base: WebToolsConfig) -> Result<WebToolsConfig, EngineError> {
    let Some(wb) = raw else { return Ok(base) };
    Ok(WebToolsConfig {
        astro_check: parse_opt_tool_policy(wb.astro_check, base.astro_check, "tools.web.astro_check")?,
        html_validate: parse_opt_tool_policy(wb.html_validate, base.html_validate, "tools.web.html_validate")?,
        wrangler_types: parse_opt_tool_policy(wb.wrangler_types, base.wrangler_types, "tools.web.wrangler_types")?,
    })
}

#[rustfmt::skip]
pub fn merge_tools(raw: Option<ToolsSection>) -> Result<ToolsConfig, EngineError> {
    let base = ToolsConfig::defaults();
    let Some(s) = raw else { return Ok(base) };
    let sh = parse_opt_tool_policy(s.shell.and_then(|x| x.shellcheck), base.shell.shellcheck, "tools.shell.shellcheck")?;
    let ps = parse_opt_tool_policy(s.powershell.and_then(|x| x.psscriptanalyzer), base.powershell.psscriptanalyzer, "tools.powershell.psscriptanalyzer")?;
    let dk = parse_opt_tool_policy(s.docker.and_then(|x| x.hadolint), base.docker.hadolint, "tools.docker.hadolint")?;
    let cs = parse_opt_tool_policy(s.css.and_then(|x| x.stylelint), base.css.stylelint, "tools.css.stylelint")?;
    let tm = parse_opt_tool_policy(s.toml.and_then(|x| x.taplo), base.toml.taplo, "tools.toml.taplo")?;
    let xm = parse_opt_tool_policy(s.xml.and_then(|x| x.xmllint), base.xml.xmllint, "tools.xml.xmllint")?;
    Ok(ToolsConfig {
        python: merge_py_tools(s.python, base.python)?,
        typescript: merge_ts_tools(s.typescript, base.typescript)?,
        shell: ShellToolsConfig { shellcheck: sh },
        powershell: PowerShellToolsConfig { psscriptanalyzer: ps },
        docker: DockerToolsConfig { hadolint: dk },
        yaml: merge_yaml_tools(s.yaml, base.yaml)?,
        css: CssToolsConfig { stylelint: cs },
        markdown: merge_md_tools(s.markdown, base.markdown)?,
        toml: TomlToolsConfig { taplo: tm },
        xml: XmlToolsConfig { xmllint: xm },
        sql: merge_sql_tools(s.sql, base.sql)?,
        web: merge_web_tools(s.web, base.web)?,
        go: merge_go_tools(s.go, base.go)?,
        kotlin: merge_kotlin_tools(s.kotlin, base.kotlin)?,
        csharp: merge_csharp_tools(s.csharp, base.csharp)?,
    })
}
