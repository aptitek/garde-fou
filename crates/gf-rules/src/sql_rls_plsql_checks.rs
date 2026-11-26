//! Cross-file PostgreSQL RLS/Policy parity and Oracle PL/SQL `PACKAGE`/`PACKAGE BODY` parity (`GF080`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Diagnostic, PostgresAdvViolation, ProceduralSqlViolation, RepoPath, Span, SqlRole, UastDocument,
};

use crate::checks::{SourceLoc, rule_error};

struct PgRlsMaps<'a> {
    enabled: BTreeMap<&'a str, &'a RepoPath>,
    policies: BTreeMap<&'a str, (&'a RepoPath, Span)>,
}

fn collect_pg_rls_maps(docs: &[UastDocument]) -> PgRlsMaps<'_> {
    let mut maps = PgRlsMaps {
        enabled: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    for doc in docs {
        let Some(sql) = &doc.sql_doc else { continue };
        if matches!(sql.role, SqlRole::Migration { is_down: true, .. }) {
            continue;
        }
        for tbl in &sql.rls_enabled_tables {
            maps.enabled.entry(tbl.as_str()).or_insert(&doc.path);
        }
        for (tbl, spans) in &sql.rls_policies_by_table {
            if let Some(&sp) = spans.first() {
                maps.policies.entry(tbl.as_str()).or_insert((&doc.path, sp));
            }
        }
    }
    maps
}

pub fn check_workspace_pg_rls(docs: &[UastDocument], diags: &mut Vec<Diagnostic>) {
    let maps = collect_pg_rls_maps(docs);
    let no_pol = PostgresAdvViolation::RlsWithoutPolicy;
    for (tbl, path) in &maps.enabled {
        if !maps.policies.contains_key(tbl) {
            let msg = format!(
                "[{}] table `{tbl}` enables RLS with zero `CREATE POLICY`",
                no_pol.code()
            );
            diags.push(rule_error(
                "GF080",
                msg,
                no_pol.remediation(),
                SourceLoc::new(path, Span::FIRST),
            ));
        }
    }
    let no_rls = PostgresAdvViolation::PolicyWithoutRls;
    for (tbl, (path, sp)) in &maps.policies {
        if !maps.enabled.contains_key(tbl) {
            let msg = format!(
                "[{}] `CREATE POLICY` on `{tbl}` without `ENABLE ROW LEVEL SECURITY`",
                no_rls.code()
            );
            diags.push(rule_error(
                "GF080",
                msg,
                no_rls.remediation(),
                SourceLoc::new(path, *sp),
            ));
        }
    }
}

struct PlsqlPkgMaps<'a> {
    specs: BTreeMap<&'a str, (&'a RepoPath, BTreeSet<&'a str>)>,
    bodies: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

fn collect_plsql_pkg_maps(docs: &[UastDocument]) -> PlsqlPkgMaps<'_> {
    let mut maps = PlsqlPkgMaps {
        specs: BTreeMap::new(),
        bodies: BTreeMap::new(),
    };
    for doc in docs {
        let Some(sql) = &doc.sql_doc else { continue };
        for (pkg, members) in &sql.plsql_package_specs {
            let entry = maps
                .specs
                .entry(pkg.as_str())
                .or_insert_with(|| (&doc.path, BTreeSet::new()));
            entry.1.extend(members.iter().map(String::as_str));
        }
        for (pkg, members) in &sql.plsql_package_bodies {
            maps.bodies
                .entry(pkg.as_str())
                .or_default()
                .extend(members.iter().map(String::as_str));
        }
    }
    maps
}

pub fn check_workspace_plsql_packages(docs: &[UastDocument], diags: &mut Vec<Diagnostic>) {
    let maps = collect_plsql_pkg_maps(docs);
    let v = ProceduralSqlViolation::PackageSpecMismatch;
    for (pkg, (spec_path, spec_members)) in maps.specs {
        let Some(body_members) = maps.bodies.get(pkg) else {
            continue;
        };
        for member in spec_members {
            if !body_members.contains(member) {
                let msg = format!(
                    "[{}] PL/SQL package `{pkg}` declares `{member}` without body impl",
                    v.code()
                );
                diags.push(rule_error(
                    "GF080",
                    msg,
                    v.remediation(),
                    SourceLoc::new(spec_path, Span::FIRST),
                ));
            }
        }
    }
}
