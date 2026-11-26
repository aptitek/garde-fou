//! Pure JSON output parser for `Spyglass` Minecraft Data Pack diagnostics (`GF082`).

use gf_core::{ExternalToolFinding, ExternalToolKind, RepoPath};
use serde::Deserialize;

use crate::tool_output::ToolParseError;

#[derive(Deserialize)]
struct SpyglassItem {
    #[serde(default)]
    file: String,
    #[serde(default)]
    line: u32,
    #[serde(default)]
    column: u32,
    #[serde(default)]
    rule: String,
    #[serde(default)]
    message: String,
}

impl SpyglassItem {
    fn into_finding(self) -> Option<ExternalToolFinding> {
        let path = RepoPath::new(self.file.trim_start_matches("./")).ok()?;
        let native_code = if self.rule.is_empty() {
            String::from("spyglass")
        } else {
            self.rule
        };
        Some(ExternalToolFinding {
            tool: ExternalToolKind::Spyglass,
            path,
            line: self.line.max(1),
            column: self.column.max(1),
            native_code,
            message: self.message,
            remediation: String::from(
                "Fix the Spyglass Minecraft Data Pack / Brigadier diagnostic at its root cause.",
            ),
        })
    }
}

/// Parses `spyglass` JSON diagnostics array into [`ExternalToolFinding`]s.
///
/// # Errors
///
/// Returns [`ToolParseError`] if stdout is not valid JSON.
pub fn parse_spyglass_json(stdout: &str) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
    match stdout.trim() {
        "" => Ok(Vec::new()),
        body => serde_json::from_str::<Vec<SpyglassItem>>(body)
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(SpyglassItem::into_finding)
                    .collect()
            })
            .map_err(|err| ToolParseError::MalformedJson {
                tool: ExternalToolKind::Spyglass,
                detail: err.to_string(),
            }),
    }
}
