//! Tier 2 configuration auditing for `pack.mcmeta`, `fabric.mod.json`, `neoforge.mods.toml`, and `*.mixins.json` (`GF081`).

use gf_core::{
    MinecraftConfigIssue, RepoPath, Span, ToolConfigFinding, ToolConfigViolation, UastDocument,
};

fn push_mc_cfg(doc: &mut UastDocument, issue: MinecraftConfigIssue, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.tool_config_findings.push(ToolConfigFinding {
        violation: ToolConfigViolation::Minecraft(issue),
        span: sp,
        message: String::from(msg),
    });
}

fn audit_pack_mcmeta(source: &str, doc: &mut UastDocument) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(source) else {
        push_mc_cfg(
            doc,
            MinecraftConfigIssue::PackMcmetaInvalid,
            1,
            "`pack.mcmeta` is not valid JSON",
        );
        return;
    };
    let pack = val.get("pack");
    let has_fmt = pack
        .and_then(|p| p.get("pack_format"))
        .and_then(serde_json::Value::as_u64)
        .is_some();
    let has_desc = pack.and_then(|p| p.get("description")).is_some();
    if !has_fmt || !has_desc {
        push_mc_cfg(
            doc,
            MinecraftConfigIssue::PackMcmetaInvalid,
            1,
            "`pack.mcmeta` must declare integer `pack.pack_format` and `pack.description`",
        );
    }
}

fn audit_required_json_strings(source: &str, keys: &[&str], msg: &str, doc: &mut UastDocument) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(source) else {
        push_mc_cfg(
            doc,
            MinecraftConfigIssue::ModOrMixinManifestInvalid,
            1,
            "Minecraft manifest JSON is malformed",
        );
        return;
    };
    let ok = keys.iter().all(|k| {
        val.get(*k)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| !s.is_empty())
    });
    if !ok {
        push_mc_cfg(doc, MinecraftConfigIssue::ModOrMixinManifestInvalid, 1, msg);
    }
}

/// Audits Minecraft manifests (`pack.mcmeta`, `fabric.mod.json`, `quilt.mod.json`, `*.mixins.json`).
pub fn audit_minecraft_tool_config(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let name = path.file_name();
    if name == "pack.mcmeta" {
        audit_pack_mcmeta(source, doc);
    } else if name.ends_with(".mixins.json") {
        audit_required_json_strings(
            source,
            &["package", "compatibilityLevel"],
            "`*.mixins.json` must declare `package` and `compatibilityLevel`",
            doc,
        );
    } else if matches!(name, "fabric.mod.json" | "quilt.mod.json") {
        audit_required_json_strings(
            source,
            &["id", "version"],
            "`fabric.mod.json` / `quilt.mod.json` must declare `id` and `version`",
            doc,
        );
    } else if name == "neoforge.mods.toml" && !source.contains("modId") {
        push_mc_cfg(
            doc,
            MinecraftConfigIssue::ModOrMixinManifestInvalid,
            1,
            "`neoforge.mods.toml` must declare `modId`",
        );
    }
}
