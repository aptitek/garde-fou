//! Pure `#![no_std]` parser for standard `LCOV` (`lcov.info`) coverage tracefiles.

use alloc::collections::BTreeMap;
use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Line coverage counters for a single instrumented source file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileCoverage {
    /// Total instrumented lines (`LF` / distinct `DA` lines).
    pub lines_found: u32,
    /// Instrumented lines executed at least once (`LH` / `DA` with hits > 0).
    pub lines_hit: u32,
}

impl FileCoverage {
    /// Integer percentage (`0..=100`) of instrumented lines executed.
    #[must_use]
    pub fn percent_floor(self) -> u32 {
        if self.lines_found == 0 {
            return 100;
        }
        let num = u64::from(self.lines_hit).saturating_mul(100);
        let pct = (num / u64::from(self.lines_found)).min(100);
        u32::try_from(pct).unwrap_or(100)
    }
}

/// Aggregated deterministic summary of an `LCOV` coverage report.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LcovSummary {
    /// Total instrumented lines across all records.
    pub total_lines_found: u32,
    /// Total executed lines across all records.
    pub total_lines_hit: u32,
    /// Per-file coverage keyed by normalized source path.
    pub files: BTreeMap<String, FileCoverage>,
}

impl LcovSummary {
    /// Integer line-coverage percentage (`0..=100`) across the entire report.
    #[must_use]
    pub fn line_coverage_pct(&self) -> u32 {
        if self.total_lines_found == 0 {
            return 0;
        }
        FileCoverage {
            lines_found: self.total_lines_found,
            lines_hit: self.total_lines_hit,
        }
        .percent_floor()
    }
}

#[derive(Default)]
struct RecordState {
    current_file: Option<String>,
    da_hits: BTreeMap<u32, u64>,
    fallback_lf: u32,
    fallback_lh: u32,
}

fn normalize_sf_path(raw: &str) -> String {
    let forward = raw.trim().replace('\\', "/");
    String::from(forward.trim_start_matches("./"))
}

fn parse_da_line(rest: &str, da_hits: &mut BTreeMap<u32, u64>) {
    let mut parts = rest.split(',');
    let (Some(line_str), Some(hits_str)) = (parts.next(), parts.next()) else {
        return;
    };
    if let (Ok(line_no), Ok(hits)) = (
        line_str.trim().parse::<u32>(),
        hits_str.trim().parse::<u64>(),
    ) && line_no > 0
    {
        let entry = da_hits.entry(line_no).or_insert(0);
        *entry = entry.saturating_add(hits);
    }
}

fn flush_record(
    rec: &mut RecordState,
    da_by_file: &mut BTreeMap<String, BTreeMap<u32, u64>>,
    fallbacks: &mut BTreeMap<String, FileCoverage>,
) {
    let Some(path) = rec.current_file.take() else {
        *rec = RecordState::default();
        return;
    };
    if !rec.da_hits.is_empty() {
        let target = da_by_file.entry(path).or_default();
        for (line_no, hits) in core::mem::take(&mut rec.da_hits) {
            let entry = target.entry(line_no).or_insert(0);
            *entry = entry.saturating_add(hits);
        }
    } else if rec.fallback_lf > 0 {
        let entry = fallbacks.entry(path).or_default();
        entry.lines_found = entry.lines_found.saturating_add(rec.fallback_lf);
        entry.lines_hit = entry
            .lines_hit
            .saturating_add(rec.fallback_lh.min(rec.fallback_lf));
    }
    *rec = RecordState::default();
}

fn build_lcov_summary(
    da_by_file: BTreeMap<String, BTreeMap<u32, u64>>,
    fallback_by_file: BTreeMap<String, FileCoverage>,
) -> LcovSummary {
    let mut summary = LcovSummary::default();
    for (path, line_map) in da_by_file {
        let found = u32::try_from(line_map.len()).unwrap_or(u32::MAX);
        let hit = u32::try_from(line_map.values().filter(|&&h| h > 0).count()).unwrap_or(u32::MAX);
        summary.total_lines_found = summary.total_lines_found.saturating_add(found);
        summary.total_lines_hit = summary.total_lines_hit.saturating_add(hit);
        summary.files.insert(
            path,
            FileCoverage {
                lines_found: found,
                lines_hit: hit,
            },
        );
    }
    for (path, cov) in fallback_by_file {
        if !summary.files.contains_key(&path) {
            summary.total_lines_found = summary.total_lines_found.saturating_add(cov.lines_found);
            summary.total_lines_hit = summary.total_lines_hit.saturating_add(cov.lines_hit);
            summary.files.insert(path, cov);
        }
    }
    summary
}

/// Parses an `LCOV` tracefile string into a deterministic [`LcovSummary`] without panicking.
#[must_use]
pub fn parse_lcov(input: &str) -> LcovSummary {
    let mut rec = RecordState::default();
    let mut da_by_file: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut fallback_by_file: BTreeMap<String, FileCoverage> = BTreeMap::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line == "end_of_record" {
            flush_record(&mut rec, &mut da_by_file, &mut fallback_by_file);
        } else if let Some(sf) = line.strip_prefix("SF:") {
            rec.current_file = Some(normalize_sf_path(sf));
        } else if let Some(da) = line.strip_prefix("DA:") {
            parse_da_line(da, &mut rec.da_hits);
        } else if let Some(lf) = line.strip_prefix("LF:") {
            rec.fallback_lf = lf.trim().parse::<u32>().unwrap_or(0);
        } else if let Some(lh) = line.strip_prefix("LH:") {
            rec.fallback_lh = lh.trim().parse::<u32>().unwrap_or(0);
        }
    }
    flush_record(&mut rec, &mut da_by_file, &mut fallback_by_file);
    build_lcov_summary(da_by_file, fallback_by_file)
}
