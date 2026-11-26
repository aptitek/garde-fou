//! Pure Safe Rust Minecraft Modding (`Kilt`/`Fabric`/`Quilt`/`NeoForge`/`Mixin`) and `.mcfunction` 1.21+ analyzers (`GF080`–`GF081`).

pub mod manifests;
pub mod mcfunction;

use gf_core::{LangRuleFinding, Language, MinecraftModViolation, RepoPath, Span, UastDocument};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};

fn push_mcmod(doc: &mut UastDocument, v: MinecraftModViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn is_minecraft_mod_source(source: &str) -> bool {
    [
        "net.minecraft.",
        "org.spongepowered.asm.mixin",
        "net.fabricmc.",
        "net.neoforged.",
        "xyz.bluspring.kilt",
        "@Mixin(",
    ]
    .iter()
    .any(|m| source.contains(m))
}

fn check_mcmod_line(trimmed: &str, ln: u32, is_client: bool, doc: &mut UastDocument) {
    if trimmed.contains("@Overwrite") {
        push_mcmod(
            doc,
            MinecraftModViolation::MixinOverwrite,
            ln,
            "`@Overwrite` Mixin breaks cross-mod compatibility; use `@Inject` or MixinExtras `@WrapOperation`",
        );
    }
    if !is_client && trimmed.contains("net.minecraft.client.") {
        push_mcmod(
            doc,
            MinecraftModViolation::ClientOnlyClassInCommon,
            ln,
            "client-only `net.minecraft.client.*` referenced in common/server source crashes dedicated servers",
        );
    }
    if trimmed.contains("Thread.sleep(") {
        push_mcmod(
            doc,
            MinecraftModViolation::ServerThreadBlock,
            ln,
            "`Thread.sleep()` on Minecraft tick thread freezes TPS/FPS",
        );
    }
    if trimmed.contains("@Redirect(") {
        push_mcmod(
            doc,
            MinecraftModViolation::UnsafeMixinTarget,
            ln,
            "brittle Mixin `@Redirect`; prefer MixinExtras `@WrapOperation` for mod compatibility",
        );
    }
}

/// Runs Minecraft Modding & Mixin/Kilt guardrails on Kotlin/JVM mod source files (`MCMOD-*`).
pub fn audit_kotlin_minecraft_mod(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    if !is_minecraft_mod_source(source) {
        return;
    }
    let is_client = path.as_str().contains("/client/") || path.as_str().contains("Client");
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            check_mcmod_line(trimmed, u32::try_from(idx + 1).unwrap_or(1), is_client, doc);
        }
    }
}

/// Pure Safe Rust extractor for Minecraft 1.21+ `.mcfunction` Data Pack files.
#[derive(Clone, Copy, Debug, Default)]
pub struct McFunctionExtractor;

impl LanguageExtractor for McFunctionExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total, effective) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::McFunction, total, effective)?;
        mcfunction::analyze_mcfunction(path, source, &mut doc);
        Ok(doc)
    }
}
