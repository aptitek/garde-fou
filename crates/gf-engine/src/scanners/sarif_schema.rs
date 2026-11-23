//! Deserialization schema and OASIS `SARIF` v2.1.0 §3.27.10 level/location accessors.

use gf_core::Span;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SarifLog {
    #[serde(default)]
    pub runs: Vec<SarifRun>,
}

#[derive(Deserialize)]
pub struct SarifRun {
    tool: Option<SarifTool>,
    #[serde(default)]
    pub results: Vec<SarifResult>,
}

impl SarifRun {
    /// Resolves effective result severity, inheriting `defaultConfiguration.level` from driver rules.
    pub fn resolve_level<'a>(&'a self, result: &'a SarifResult) -> &'a str {
        if result.kind.as_deref().is_some_and(|k| k != "fail") {
            return "none";
        }
        if let Some(lvl) = result.level.as_deref().filter(|s| !s.is_empty()) {
            return lvl;
        }
        let rules = self
            .tool
            .as_ref()
            .map_or(&[][..], |t| t.driver.rules.as_slice());
        let matched = result
            .rule_index
            .and_then(|idx| rules.get(idx))
            .or_else(|| {
                let rid = result.rule_id.as_deref()?;
                rules.iter().find(|r| r.id == rid)
            });
        matched
            .and_then(|r| r.default_configuration.as_ref())
            .and_then(|cfg| cfg.level.as_deref())
            .unwrap_or("warning")
    }
}

#[derive(Deserialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Deserialize)]
struct SarifDriver {
    #[serde(default)]
    rules: Vec<SarifRule>,
}

#[derive(Deserialize)]
struct SarifRule {
    #[serde(default)]
    id: String,
    #[serde(rename = "defaultConfiguration")]
    default_configuration: Option<SarifDefaultConfig>,
}

#[derive(Deserialize)]
struct SarifDefaultConfig {
    level: Option<String>,
}

#[derive(Deserialize)]
pub struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    #[serde(rename = "ruleIndex")]
    rule_index: Option<usize>,
    kind: Option<String>,
    level: Option<String>,
    message: Option<SarifMessage>,
    #[serde(default)]
    locations: Vec<SarifLocation>,
}

impl SarifResult {
    pub fn external_rule_id(&self) -> &str {
        self.rule_id.as_deref().unwrap_or("finding")
    }

    pub fn message_text(&self) -> &str {
        self.message
            .as_ref()
            .map_or("security finding detected", |m| m.text.as_str())
    }

    pub fn primary_uri(&self) -> Option<&str> {
        self.locations
            .first()
            .and_then(|l| l.physical_location.as_ref())
            .and_then(|p| p.artifact_location.as_ref())
            .and_then(|a| a.uri.as_deref())
    }

    pub fn primary_span(&self) -> Span {
        let region = self
            .locations
            .first()
            .and_then(|l| l.physical_location.as_ref())
            .and_then(|p| p.region.as_ref());
        let start = region.and_then(|r| r.start_line).unwrap_or(1).max(1);
        let end = region.and_then(|r| r.end_line).unwrap_or(start).max(start);
        Span::new(start, end).unwrap_or(Span::FIRST)
    }
}

#[derive(Deserialize)]
struct SarifMessage {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: Option<SarifPhysicalLocation>,
}

#[derive(Deserialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: Option<SarifArtifactLocation>,
    region: Option<SarifRegion>,
}

#[derive(Deserialize)]
struct SarifArtifactLocation {
    uri: Option<String>,
}

#[derive(Deserialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: Option<u32>,
    #[serde(rename = "endLine")]
    end_line: Option<u32>,
}
