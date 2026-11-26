//! Pure `#![no_std]` Tier 2 & Tier 3 configuration types for Minecraft Modding & Data Packs (`Spyglass`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::external_tool_kind::ExternalToolKind;
use crate::policies::ScannerPolicy;

/// Tier 2 Minecraft manifest & Mixin configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MinecraftConfigIssue {
    /// `pack.mcmeta` missing `pack.pack_format` or `pack.description`.
    PackMcmetaInvalid,
    /// `fabric.mod.json` / `quilt.mod.json` / `neoforge.mods.toml` / `*.mixins.json` missing required fields.
    ModOrMixinManifestInvalid,
}

impl MinecraftConfigIssue {
    /// Affected external tool.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        ExternalToolKind::Spyglass
    }

    /// Dotted configuration key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::PackMcmetaInvalid => "pack.pack_format",
            Self::ModOrMixinManifestInvalid => "mixins.compatibilityLevel",
        }
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::PackMcmetaInvalid => {
                "Declare integer `pack.pack_format` (e.g. `48`+ for 1.21+) and non-empty `pack.description` in `pack.mcmeta`."
            }
            Self::ModOrMixinManifestInvalid => {
                "Declare required `id`/`version` in mod manifests and `package`/`compatibilityLevel` in `*.mixins.json`."
            }
        }
    }
}

/// `[tools.minecraft]` execution policy for `spyglass` datapack verification (`GF082`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinecraftToolsConfig {
    /// Policy for `spyglass` (`auto` by default).
    pub spyglass: ScannerPolicy,
}

impl MinecraftToolsConfig {
    /// Default Minecraft tool policy (`spyglass = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            spyglass: ScannerPolicy::Auto,
        }
    }

    /// Deterministic cache-key fragment.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("mc({})", self.spyglass.as_str())
    }
}

impl Default for MinecraftToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` for Minecraft Tier 2 audited manifests (`pack.mcmeta`, `fabric.mod.json`, `*.mixins.json`).
#[must_use]
pub fn is_minecraft_tool_config_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "pack.mcmeta" | "fabric.mod.json" | "quilt.mod.json" | "neoforge.mods.toml"
    ) || name.ends_with(".mixins.json")
}
