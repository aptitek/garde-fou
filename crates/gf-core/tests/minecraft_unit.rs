//! Unit & mutation-killing tests for Minecraft Modding (Kilt/Fabric/NeoForge) and `.mcfunction` core types.

use gf_core::{
    ExternalToolKind, FrameworkId, LangRuleViolation, LanguageId, McFunctionViolation,
    MinecraftConfigIssue, MinecraftModViolation, MinecraftToolsConfig, MinecraftViolation,
    ToolConfigViolation, is_minecraft_tool_config_file,
};

#[test]
fn minecraft_violation_codes_and_remediations_are_unique() {
    let all = [
        MinecraftViolation::Mod(MinecraftModViolation::MixinOverwrite),
        MinecraftViolation::Mod(MinecraftModViolation::ClientOnlyClassInCommon),
        MinecraftViolation::Mod(MinecraftModViolation::ServerThreadBlock),
        MinecraftViolation::Mod(MinecraftModViolation::UnsafeMixinTarget),
        MinecraftViolation::Function(McFunctionViolation::LeadingSlashCommand),
        MinecraftViolation::Function(McFunctionViolation::InvalidMacroSyntax),
        MinecraftViolation::Function(McFunctionViolation::UnboundedEntitySelector),
        MinecraftViolation::Function(McFunctionViolation::LegacyPluralFolder121),
        MinecraftViolation::Function(McFunctionViolation::UnsafeAdminCommand),
    ];
    let mut codes = std::collections::BTreeSet::new();
    let mut rems = std::collections::BTreeSet::new();
    for v in all {
        assert_ne!(v.code(), "");
        assert!(v.remediation().len() >= 20);
        assert_eq!(format!("{v}"), v.code());
        assert!(codes.insert(v.code()), "duplicate code {}", v.code());
        assert!(rems.insert(v.remediation()), "duplicate remediation");
        let wrapped = LangRuleViolation::from(v);
        assert_eq!(wrapped.code(), v.code());
        assert_eq!(wrapped.remediation(), v.remediation());
        assert!(!wrapped.is_entrypoint_exempt());
    }
    assert_eq!(codes.len(), 9);
    assert_eq!(rems.len(), 9);
}

#[test]
fn minecraft_tools_config_and_context_slugs_resolve() {
    assert_eq!(
        LanguageId::from_slug("mcfunction"),
        Some(LanguageId::McFunction)
    );
    assert_eq!(
        FrameworkId::from_config_str("kilt"),
        Some(FrameworkId::Kilt)
    );
    assert_eq!(
        FrameworkId::from_config_str("minecraft-mod"),
        Some(FrameworkId::MinecraftMod)
    );
    assert_eq!(
        FrameworkId::from_config_str("datapack"),
        Some(FrameworkId::DataPack)
    );
    assert_eq!(ExternalToolKind::Spyglass.as_str(), "spyglass");
    assert!(is_minecraft_tool_config_file("pack.mcmeta"));
    assert!(is_minecraft_tool_config_file("fabric.mod.json"));
    assert!(is_minecraft_tool_config_file("mymod.mixins.json"));
    assert!(!is_minecraft_tool_config_file("src/main/kotlin/Mod.kt"));
    let cfg = MinecraftToolsConfig::default();
    assert_eq!(cfg.canonical_fragment(), "mc(auto)");
    for (issue, exp_key) in [
        (MinecraftConfigIssue::PackMcmetaInvalid, "pack.pack_format"),
        (
            MinecraftConfigIssue::ModOrMixinManifestInvalid,
            "mixins.compatibilityLevel",
        ),
    ] {
        let tv = ToolConfigViolation::Minecraft(issue);
        assert_eq!(tv.tool(), ExternalToolKind::Spyglass);
        assert_eq!(issue.key(), exp_key);
        assert_eq!(tv.key(), exp_key);
        assert!(issue.remediation().len() >= 20);
        assert_eq!(tv.remediation(), issue.remediation());
    }
}
