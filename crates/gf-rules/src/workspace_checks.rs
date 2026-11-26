//! Workspace entrypoint classification and public / crate-internal dead export detection (`GF021`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    ArchetypePolicy, Config, DeadExportGate, Diagnostic, DomainKind, ExportDecl, Language,
    ProjectContext, RepoPath, Severity, UastDocument, Validation, classify_path, glob_matches,
    is_django_migration_path, is_ts_declaration_path,
};

const UNIVERSAL_ENTRYPOINT_GLOBS: &[&str] = &[
    "src/lib.rs",
    "src/main.*",
    "crates/*/src/lib.rs",
    "crates/*/src/main.rs",
    "src/index.*",
    "main.*",
    "index.*",
    "__init__.py",
    "src/*/__init__.py",
    "tests/**",
    "**/tests/**",
    "*.test.*",
    "**/*.test.*",
    "*.spec.*",
    "**/*.spec.*",
    "*.stories.*",
    "**/*.stories.*",
    "*_test.*",
    "**/*_test.*",
    "src/pages/**",
    "src/layouts/**",
    "src/middleware.*",
    "src/content.config.*",
    "src/content/config.*",
    "astro.config.*",
    "app/root.*",
    "src/root.*",
    "root.tsx",
    "app/routes.*",
    "src/routes.*",
    "app/routes/**",
    "src/routes/**",
    "**/routes/**",
    "react-router.config.*",
    "panda.config.*",
    "drizzle.config.*",
    "src/worker.*",
    "worker.*",
    "cmd/**",
    "Program.cs",
    "**/Program.cs",
    "**/Main.kt",
    "*.kts",
    "**/*.kts",
    "*Test.kt",
    "**/*Test.kt",
    "*Tests.cs",
    "**/*Tests.cs",
];

pub fn is_entrypoint_file(path: &RepoPath, ctx: &ProjectContext, config: &Config) -> bool {
    let raw = path.as_str();
    UNIVERSAL_ENTRYPOINT_GLOBS
        .iter()
        .any(|pat| glob_matches(pat, raw))
        || config.entrypoints.iter().any(|pat| glob_matches(pat, raw))
        || ctx.frameworks.iter().any(|fw| {
            fw.implicit_entrypoints()
                .iter()
                .any(|pat| glob_matches(pat, raw))
        })
}

fn crate_scope_prefix(path: &RepoPath) -> &str {
    let raw = path.as_str();
    if let Some(after) = raw.strip_prefix("crates/")
        && let Some((crate_name, _)) = after.split_once('/')
    {
        return &raw[..=(7 + crate_name.len())];
    }
    ""
}

fn build_reference_index(docs: &[UastDocument]) -> BTreeMap<&str, BTreeSet<&RepoPath>> {
    let mut index: BTreeMap<&str, BTreeSet<&RepoPath>> = BTreeMap::new();
    for doc in docs {
        for sym in &doc.referenced_symbols {
            index.entry(sym.as_str()).or_default().insert(&doc.path);
        }
        for imp in &doc.imports {
            for sym in &imp.symbols {
                index.entry(sym.as_str()).or_default().insert(&doc.path);
            }
        }
    }
    index
}

fn is_export_used(
    export: &ExportDecl,
    owner_path: &RepoPath,
    ref_index: &BTreeMap<&str, BTreeSet<&RepoPath>>,
) -> bool {
    let Some(paths) = ref_index.get(export.symbol.as_str()) else {
        return false;
    };
    if export.is_crate_internal {
        let own_crate = crate_scope_prefix(owner_path);
        paths
            .iter()
            .any(|p| *p != owner_path && crate_scope_prefix(p) == own_crate)
    } else {
        paths.iter().any(|p| *p != owner_path)
    }
}

fn check_doc_dead_exports(
    doc: &UastDocument,
    is_entry: bool,
    ref_index: &BTreeMap<&str, BTreeSet<&RepoPath>>,
    out: &mut Vec<Diagnostic>,
) {
    for export in &doc.exports {
        if (!export.is_crate_internal && is_entry) || is_export_used(export, &doc.path, ref_index) {
            continue;
        }
        let scope_label = if export.is_crate_internal {
            "crate-internal (`pub(crate)`)"
        } else {
            "public"
        };
        out.push(Diagnostic {
            rule_id: String::from("GF021"),
            severity: Severity::Error,
            message: format!(
                "dead {scope_label} export `{}` in `{}` is unused",
                export.symbol, doc.path
            ),
            remediation: String::from(
                "Narrow symbol visibility to module-private (remove `pub` / `pub(crate)` / `export`) or delete genuinely unused code.",
            ),
            measured: 1,
            threshold: 0,
            path: doc.path.clone(),
            span: export.span,
        });
    }
}

/// Detects dead `pub`/`export` symbols and unused `pub(crate)` exports (`GF021`).
pub fn check_dead_exports(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    if ArchetypePolicy::for_archetype(ctx.archetype).dead_export_gate != DeadExportGate::Enforced {
        return Validation::Valid(());
    }
    let ref_index = build_reference_index(docs);
    let mut diags = Vec::new();
    for doc in docs {
        if classify_path(&doc.path, &config.domains) == DomainKind::Code
            && !matches!(
                doc.language,
                Language::Dockerfile | Language::Notebook | Language::Quarto
            )
            && !is_ts_declaration_path(doc.path.as_str())
            && !is_django_migration_path(doc.path.as_str())
        {
            let is_entry = is_entrypoint_file(&doc.path, ctx, config);
            check_doc_dead_exports(doc, is_entry, &ref_index, &mut diags);
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
