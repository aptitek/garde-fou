//! Symmetric corpus tests for Kilt/Fabric/NeoForge Kotlin mods, `.mcfunction` 1.21+, manifests, and Spyglass.

use gf_core::{ExternalToolKind, LangRuleFinding, RepoPath};
use gf_parsers::{parse_source, parse_spyglass_json};

#[test]
fn minecraft_kotlin_mod_bad_and_clean_corpus() -> Result<(), String> {
    let common_p =
        RepoPath::new("src/main/kotlin/com/demo/ServerMixin.kt").map_err(|e| e.to_string())?;
    let bad_kt = r#"
import net.minecraft.client.MinecraftClient
import org.spongepowered.asm.mixin.Mixin
@Mixin(ServerWorld::class)
class BadMixin {
    @Overwrite
    fun tick() { Thread.sleep(50) }
    @Redirect(method = ["tick"])
    fun redir() {}
}
"#;
    let doc = parse_source(&common_p, bad_kt).map_err(|e| e.to_string())?;
    let codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    for exp in [
        "MCMOD-MIXIN-OVERWRITE",
        "MCMOD-CLIENT-IN-COMMON",
        "MCMOD-SERVER-THREAD-BLOCK",
        "MCMOD-UNSAFE-MIXIN",
    ] {
        assert!(codes.contains(&exp), "missing {exp} in {codes:?}");
    }

    let client_p =
        RepoPath::new("src/client/kotlin/com/demo/HudMixin.kt").map_err(|e| e.to_string())?;
    let clean_kt = r#"
import net.minecraft.client.MinecraftClient
import org.spongepowered.asm.mixin.Mixin
@Mixin(MinecraftClient::class)
class HudMixin {
    @Inject(method = ["render"])
    fun demo_onRender() {}
}
"#;
    let clean = parse_source(&client_p, clean_kt).map_err(|e| e.to_string())?;
    assert!(
        !clean
            .lang_rule_findings
            .iter()
            .any(|f| f.code().starts_with("MCMOD-")),
        "unexpected MCMOD findings: {:?}",
        clean.lang_rule_findings
    );
    Ok(())
}

#[test]
fn mcfunction_121_manifests_and_spyglass_corpus() -> Result<(), String> {
    let bad_p = RepoPath::new("data/demo/functions/tick.mcfunction").map_err(|e| e.to_string())?;
    let bad_mc = "/say hi\n$say no_param\nsay $(missing_dollar)\nkill @e\nstop\n";
    let doc = parse_source(&bad_p, bad_mc).map_err(|e| e.to_string())?;
    let codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    for exp in [
        "MCFUNC-121-PLURAL-FOLDER",
        "MCFUNC-LEADING-SLASH",
        "MCFUNC-MACRO-SYNTAX",
        "MCFUNC-UNBOUNDED-SELECTOR",
        "MCFUNC-UNSAFE-ADMIN-CMD",
    ] {
        assert!(codes.contains(&exp), "missing {exp} in {codes:?}");
    }

    let ok_p = RepoPath::new("data/demo/function/tick.mcfunction").map_err(|e| e.to_string())?;
    let ok_mc = "# 1.21 tick\n$say Hello $(player)\nkill @e[type=minecraft:zombie,limit=10]\nfunction demo:helper\n";
    let ok_doc = parse_source(&ok_p, ok_mc).map_err(|e| e.to_string())?;
    assert_eq!(ok_doc.lang_rule_findings.len(), 0);
    assert_eq!(ok_doc.imports.len(), 1);

    let mcmeta_p = RepoPath::new("pack.mcmeta").map_err(|e| e.to_string())?;
    let bad_meta = parse_source(&mcmeta_p, "{}").map_err(|e| e.to_string())?;
    assert_eq!(bad_meta.tool_config_findings.len(), 1);

    let mix_p = RepoPath::new("demo.mixins.json").map_err(|e| e.to_string())?;
    let bad_mix = parse_source(&mix_p, "{}").map_err(|e| e.to_string())?;
    assert_eq!(bad_mix.tool_config_findings.len(), 1);

    let spy_json = r#"[{"file":"data/demo/function/tick.mcfunction","line":3,"column":1,"rule":"mcfunction/unknown-command","message":"Unknown command"}]"#;
    let findings = parse_spyglass_json(spy_json).map_err(|e| e.to_string())?;
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool, ExternalToolKind::Spyglass);
    Ok(())
}
