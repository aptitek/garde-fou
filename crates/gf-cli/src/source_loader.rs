//! Lazy disk loader reading exclusively diagnosed files (zero I/O on clean files)
//! and optional `LCOV` coverage percentage extraction.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use gf_core::{is_binary_asset_extension, parse_lcov};
use gf_engine::CheckReport;

use crate::ui::SourceFileView;

/// Loads [`SourceFileView`] entries ONLY for paths referenced in `report.diagnostics`.
///
/// Clean files incur zero filesystem reads; binary assets and non-UTF8 files are marked
/// [`SourceFileView::BinaryOrOmitted`].
#[must_use]
pub fn load_diagnosed_sources(
    cwd: &Path,
    report: &CheckReport,
) -> BTreeMap<String, SourceFileView> {
    if report.is_clean() {
        return BTreeMap::new();
    }
    let unique_paths: BTreeSet<&str> = report.diagnostics.iter().map(|d| d.path.as_str()).collect();

    let mut out = BTreeMap::new();
    for rel in unique_paths {
        let view = read_single_source(cwd, rel);
        out.insert(String::from(rel), view);
    }
    out
}

fn read_single_source(cwd: &Path, rel: &str) -> SourceFileView {
    let ext = Path::new(rel).extension().and_then(|s| s.to_str());
    if ext.is_some_and(is_binary_asset_extension) {
        return SourceFileView::BinaryOrOmitted;
    }
    let full = cwd.join(rel);
    let Ok(bytes) = fs::read(full) else {
        return SourceFileView::Unavailable;
    };
    if bytes.contains(&0) {
        return SourceFileView::BinaryOrOmitted;
    }
    String::from_utf8(bytes).map_or(SourceFileView::BinaryOrOmitted, SourceFileView::Text)
}

/// Computes `LCOV` line coverage in tenths of a percent (`0..=1000`) when `--lcov` is supplied.
#[must_use]
pub fn read_lcov_tenths(cwd: &Path, lcov_arg: Option<&str>) -> Option<u32> {
    let rel = lcov_arg?;
    let target = Path::new(rel);
    let full = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    let raw = fs::read_to_string(full).ok()?;
    let summary = parse_lcov(&raw);
    if summary.total_lines_found == 0 {
        return Some(1000);
    }
    let scaled = (u64::from(summary.total_lines_hit) * 1000) / u64::from(summary.total_lines_found);
    Some(u32::try_from(scaled.min(1000)).unwrap_or(1000))
}
