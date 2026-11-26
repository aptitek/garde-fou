//! Pure `#![no_std]` evaluators for `MIG-DESTRUCTIVE-DDL` (`[sql] allow_destructive_migrations`)
//! and workspace cross-migration ordering (`MIG-VERSION-COLLISION-OR-GAP` — `GF080`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, MigrationViolation, RepoPath, Span, SqlRole, UastDocument, Validation,
};

use crate::checks::{SourceLoc, rule_error};
use crate::sql_rls_plsql_checks::{check_workspace_pg_rls, check_workspace_plsql_packages};

/// Evaluates `MIG-DESTRUCTIVE-DDL` on a single `.sql` document unless explicitly allowed.
pub fn check_local_sql_destructive_ddl(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    if config.sql.allow_destructive_migrations {
        return Validation::Valid(());
    }
    let Some(sql) = &doc.sql_doc else {
        return Validation::Valid(());
    };
    let v = MigrationViolation::DestructiveDdl;
    let diags = sql
        .destructive_ddl
        .iter()
        .map(|d| {
            rule_error(
                "GF080",
                format!("[{}] {}", v.code(), d.detail),
                v.remediation(),
                SourceLoc::new(&doc.path, d.span),
            )
        })
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}

struct GroupMigrations<'a> {
    up_by_version: BTreeMap<&'a str, Vec<&'a RepoPath>>,
    up_stems: BTreeSet<&'a str>,
    downs: Vec<(&'a RepoPath, &'a str)>,
}

impl GroupMigrations<'_> {
    const fn new() -> Self {
        Self {
            up_by_version: BTreeMap::new(),
            up_stems: BTreeSet::new(),
            downs: Vec::new(),
        }
    }
}

fn collect_migration_groups(docs: &[UastDocument]) -> BTreeMap<&str, GroupMigrations<'_>> {
    let mut groups: BTreeMap<&str, GroupMigrations<'_>> = BTreeMap::new();
    for doc in docs {
        let Some(sql) = &doc.sql_doc else {
            continue;
        };
        let SqlRole::Migration {
            version_prefix,
            migration_stem,
            is_down,
        } = &sql.role
        else {
            continue;
        };
        let group = groups
            .entry(sql.migration_group_dir.as_str())
            .or_insert_with(GroupMigrations::new);
        if *is_down {
            group.downs.push((&doc.path, migration_stem.as_str()));
        } else {
            group.up_stems.insert(migration_stem.as_str());
            if let Some(ver) = version_prefix.as_deref() {
                group.up_by_version.entry(ver).or_default().push(&doc.path);
            }
        }
    }
    groups
}

fn emit_version_collisions(
    group_dir: &str,
    up_by_version: BTreeMap<&str, Vec<&RepoPath>>,
    diags: &mut Vec<Diagnostic>,
) {
    let v = MigrationViolation::VersionCollisionOrGap;
    for (ver, mut paths) in up_by_version {
        if paths.len() <= 1 {
            continue;
        }
        paths.sort();
        let first = paths[0];
        for dup in &paths[1..] {
            diags.push(rule_error(
                "GF080",
                format!(
                    "[{}] duplicate migration version prefix `{ver}` in `{group_dir}` (`{dup}` collides with `{first}`)",
                    v.code()
                ),
                v.remediation(),
                SourceLoc::new(dup, Span::FIRST),
            ));
        }
    }
}

fn emit_orphan_downs(
    group_dir: &str,
    up_stems: &BTreeSet<&str>,
    downs: &[(&RepoPath, &str)],
    diags: &mut Vec<Diagnostic>,
) {
    let v = MigrationViolation::VersionCollisionOrGap;
    for (down_path, stem) in downs {
        if !up_stems.contains(stem) {
            diags.push(rule_error(
                "GF080",
                format!(
                    "[{}] orphan rollback migration `{down_path}` has no matching up-migration stem `{stem}` in `{group_dir}`",
                    v.code()
                ),
                v.remediation(),
                SourceLoc::new(down_path, Span::FIRST),
            ));
        }
    }
}

/// Evaluates cross-file migration collisions, orphan `.down.sql`, PostgreSQL RLS parity,
/// and Oracle PL/SQL `PACKAGE` / `PACKAGE BODY` parity (`GF080`).
pub fn check_workspace_sql_migrations(docs: &[UastDocument]) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    for (group_dir, group) in collect_migration_groups(docs) {
        emit_version_collisions(group_dir, group.up_by_version, &mut diags);
        emit_orphan_downs(group_dir, &group.up_stems, &group.downs, &mut diags);
    }
    check_workspace_pg_rls(docs, &mut diags);
    check_workspace_plsql_packages(docs, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
