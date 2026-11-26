//! Modern exclusive toolchain auditor (`pnpm`, `uv`, Gradle `.kts`) (`GF081`).

use std::collections::BTreeSet;

use gf_core::{
    RepoPath, Span, ToolConfigFinding, ToolConfigViolation, ToolchainSummary, ToolchainViolation,
    UastDocument,
};

use super::jsonc::push_cfg_violation;

fn line_span(line_no: u32) -> Span {
    let n = line_no.max(1);
    Span::new(n, n).unwrap_or(Span::FIRST)
}

fn push_tc(out: &mut Vec<ToolConfigFinding>, v: ToolchainViolation, span: Span, msg: String) {
    push_cfg_violation(out, ToolConfigViolation::Toolchain(v), span, msg);
}

fn collect_pkg_deps(root: &serde_json::Map<String, serde_json::Value>) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(obj) = root.get(key).and_then(serde_json::Value::as_object) {
            deps.extend(obj.keys().cloned());
        }
    }
    deps
}

fn is_valid_pnpm_pm(pm: &str) -> bool {
    let Some(ver) = pm.trim().strip_prefix("pnpm@") else {
        return false;
    };
    !ver.is_empty()
        && !ver.starts_with('^')
        && !ver.starts_with('~')
        && ver.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn has_legacy_js_cmd(cmd: &str) -> bool {
    for seg in cmd.split(['&', ';', '|', '\n']) {
        let s = seg.trim_start();
        if ["npm ", "npx ", "yarn "].iter().any(|p| s.starts_with(p)) || matches!(s, "npm" | "yarn")
        {
            return true;
        }
    }
    false
}

fn audit_pkg_scripts(
    root: &serde_json::Map<String, serde_json::Value>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let Some(scripts) = root.get("scripts").and_then(serde_json::Value::as_object) else {
        return;
    };
    for (name, val) in scripts {
        if val.as_str().is_some_and(has_legacy_js_cmd) {
            let line = find_line(source, name).unwrap_or(1);
            push_tc(
                out,
                ToolchainViolation::PnpmOnly,
                line_span(line),
                format!(
                    "`package.json` script `{name}` uses legacy `npm`/`npx`/`yarn`; use `pnpm`"
                ),
            );
        }
    }
}

fn audit_package_json(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let Ok(serde_json::Value::Object(root)) = serde_json::from_str::<serde_json::Value>(source)
    else {
        return;
    };
    let deps = collect_pkg_deps(&root);
    if path.as_str() == "package.json" {
        let pm = root
            .get("packageManager")
            .and_then(serde_json::Value::as_str);
        if !pm.is_some_and(is_valid_pnpm_pm) {
            push_tc(
                &mut doc.tool_config_findings,
                ToolchainViolation::PnpmOnly,
                line_span(1),
                String::from(
                    "root `package.json` must pin `\"packageManager\": \"pnpm@<exact-version>\"`",
                ),
            );
        }
    }
    audit_pkg_scripts(&root, source, &mut doc.tool_config_findings);
    let is_node_pkg = deps.contains("@types/node")
        || deps.contains("express")
        || deps.contains("mongoose")
        || deps.contains("mongodb")
        || root.get("engines").and_then(|e| e.get("node")).is_some();
    let is_esm = root.get("type").and_then(serde_json::Value::as_str) == Some("module");
    if is_node_pkg && !is_esm {
        push_tc(
            &mut doc.tool_config_findings,
            ToolchainViolation::NodeEsmModuleType,
            line_span(1),
            String::from("Node.js `package.json` must declare `\"type\": \"module\"`"),
        );
    }
    doc.toolchain_meta = Some(ToolchainSummary {
        package_json_deps: deps,
        ..ToolchainSummary::default()
    });
}

fn find_line(source: &str, needle: &str) -> Option<u32> {
    source
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX))
}

fn audit_gradle_catalog(source: &str, doc: &mut UastDocument) {
    for (idx, raw) in source.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or(raw).trim();
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        if line.contains("SNAPSHOT")
            || line.contains("latest.release")
            || line.contains(".+\"")
            || line.contains("\"+\"")
        {
            push_tc(
                &mut doc.tool_config_findings,
                ToolchainViolation::GradleKtsOnly,
                line_span(line_no),
                String::from(
                    "dynamic or `SNAPSHOT` version in `libs.versions.toml`; pin an immutable release",
                ),
            );
        }
    }
    doc.toolchain_meta = Some(ToolchainSummary {
        is_gradle_version_catalog: true,
        ..ToolchainSummary::default()
    });
}

fn audit_gradle_wrapper(source: &str, out: &mut Vec<ToolConfigFinding>) {
    let insecure_lines = source
        .lines()
        .enumerate()
        .filter(|(_, raw)| {
            let l = raw.trim();
            l.starts_with("distributionUrl") && (l.contains("http\\://") || l.contains("http://"))
        })
        .map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX));
    for line_no in insecure_lines {
        push_tc(
            out,
            ToolchainViolation::GradleKtsOnly,
            line_span(line_no),
            String::from(
                "`gradle-wrapper.properties` uses unencrypted `http://` distributionUrl; use `https://`",
            ),
        );
    }
}

/// Audits `package.json`, legacy lockfiles/manifests, `libs.versions.toml`, and `gradle-wrapper.properties`.
pub fn audit_toolchain_file(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let base = path.file_name();
    let low = base.to_ascii_lowercase();
    match low.as_str() {
        "package.json" => audit_package_json(path, source, doc),
        "libs.versions.toml" => audit_gradle_catalog(source, doc),
        "gradle-wrapper.properties" => audit_gradle_wrapper(source, &mut doc.tool_config_findings),
        "package-lock.json"
        | "npm-shrinkwrap.json"
        | "yarn.lock"
        | ".yarnrc"
        | ".yarnrc.yml"
        | ".pnp.cjs" => push_tc(
            &mut doc.tool_config_findings,
            ToolchainViolation::PnpmOnly,
            line_span(1),
            format!("legacy JS/TS artifact `{base}` is forbidden; migrate to `pnpm`"),
        ),
        "build.gradle" | "settings.gradle" => push_tc(
            &mut doc.tool_config_findings,
            ToolchainViolation::GradleKtsOnly,
            line_span(1),
            format!("Groovy Gradle file `{base}` is forbidden; migrate to `{base}.kts`"),
        ),
        _ => push_tc(
            &mut doc.tool_config_findings,
            ToolchainViolation::UvOnly,
            line_span(1),
            format!(
                "legacy Python manifest/lockfile `{base}` is forbidden; migrate to `pyproject.toml` + `uv.lock`"
            ),
        ),
    }
}
