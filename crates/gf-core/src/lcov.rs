//! Pure `#![no_std]` parser for `LCOV` (`lcov.info`) line (`DA`/`LF`/`LH`) and branch (`BRDA`/`BRF`/`BRH`) tracefiles.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;

pub use crate::coverage::{FileCoverage, LcovSummary, compute_crap_score};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BranchKey {
    line: u32,
    block: String,
    branch: String,
}

struct LineMetrics {
    found: u32,
    hit: u32,
    executed: BTreeSet<u32>,
}

#[derive(Default)]
struct FileAccumulator {
    da_hits: BTreeMap<u32, u64>,
    brda_hits: BTreeMap<BranchKey, u64>,
    fallback_lf: u32,
    fallback_lh: u32,
    fallback_brf: u32,
    fallback_brh: u32,
}

impl FileAccumulator {
    fn merge_from(&mut self, other: Self) {
        for (line_no, hits) in other.da_hits {
            let entry = self.da_hits.entry(line_no).or_insert(0);
            *entry = entry.saturating_add(hits);
        }
        for (bkey, taken) in other.brda_hits {
            let entry = self.brda_hits.entry(bkey).or_insert(0);
            *entry = entry.saturating_add(taken);
        }
        self.fallback_lf = self.fallback_lf.saturating_add(other.fallback_lf);
        self.fallback_lh = self
            .fallback_lh
            .saturating_add(other.fallback_lh.min(other.fallback_lf));
        self.fallback_brf = self.fallback_brf.saturating_add(other.fallback_brf);
        self.fallback_brh = self
            .fallback_brh
            .saturating_add(other.fallback_brh.min(other.fallback_brf));
    }

    fn into_file_coverage(self) -> FileCoverage {
        let lines = if self.da_hits.is_empty() {
            LineMetrics {
                found: self.fallback_lf,
                hit: self.fallback_lh.min(self.fallback_lf),
                executed: BTreeSet::new(),
            }
        } else {
            let executed: BTreeSet<u32> = self
                .da_hits
                .iter()
                .filter_map(|(&ln, &h)| (h > 0).then_some(ln))
                .collect();
            LineMetrics {
                found: u32::try_from(self.da_hits.len()).unwrap_or(u32::MAX),
                hit: u32::try_from(executed.len()).unwrap_or(u32::MAX),
                executed,
            }
        };

        let (branches_found, branches_hit) = if self.brda_hits.is_empty() {
            (self.fallback_brf, self.fallback_brh.min(self.fallback_brf))
        } else {
            let b_found = u32::try_from(self.brda_hits.len()).unwrap_or(u32::MAX);
            let b_hit = u32::try_from(self.brda_hits.values().filter(|&&t| t > 0).count())
                .unwrap_or(u32::MAX);
            (b_found, b_hit)
        };

        FileCoverage {
            lines_found: lines.found,
            lines_hit: lines.hit,
            branches_found,
            branches_hit,
            executed_lines: lines.executed,
            line_hits: self.da_hits,
        }
    }
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

fn parse_brda_line(rest: &str, brda_hits: &mut BTreeMap<BranchKey, u64>) {
    let mut parts = rest.splitn(4, ',');
    let Some(line_s) = parts.next() else { return };
    let Some(block_s) = parts.next() else { return };
    let Some(branch_s) = parts.next() else { return };
    let Some(taken_s) = parts.next() else { return };
    let taken_trimmed = taken_s.trim();
    if taken_trimmed == "-" {
        return;
    }
    if let (Ok(line_no), Ok(taken)) = (line_s.trim().parse::<u32>(), taken_trimmed.parse::<u64>())
        && line_no > 0
    {
        let key = BranchKey {
            line: line_no,
            block: String::from(block_s.trim()),
            branch: String::from(branch_s.trim()),
        };
        let entry = brda_hits.entry(key).or_insert(0);
        *entry = entry.saturating_add(taken);
    }
}

fn apply_record_directive(line: &str, acc: &mut FileAccumulator) {
    if let Some(da) = line.strip_prefix("DA:") {
        parse_da_line(da, &mut acc.da_hits);
    } else if let Some(brda) = line.strip_prefix("BRDA:") {
        parse_brda_line(brda, &mut acc.brda_hits);
    } else if let Some(lf) = line.strip_prefix("LF:") {
        acc.fallback_lf = lf.trim().parse::<u32>().unwrap_or(0);
    } else if let Some(lh) = line.strip_prefix("LH:") {
        acc.fallback_lh = lh.trim().parse::<u32>().unwrap_or(0);
    } else if let Some(brf) = line.strip_prefix("BRF:") {
        acc.fallback_brf = brf.trim().parse::<u32>().unwrap_or(0);
    } else if let Some(brh) = line.strip_prefix("BRH:") {
        acc.fallback_brh = brh.trim().parse::<u32>().unwrap_or(0);
    }
}

fn finalize_lcov_summary(files_acc: BTreeMap<String, FileAccumulator>) -> LcovSummary {
    let mut summary = LcovSummary::default();
    for (path, acc) in files_acc {
        let cov = acc.into_file_coverage();
        if cov.lines_found > 0 || cov.branches_found > 0 {
            summary.total_lines_found = summary.total_lines_found.saturating_add(cov.lines_found);
            summary.total_lines_hit = summary.total_lines_hit.saturating_add(cov.lines_hit);
            summary.total_branches_found = summary
                .total_branches_found
                .saturating_add(cov.branches_found);
            summary.total_branches_hit =
                summary.total_branches_hit.saturating_add(cov.branches_hit);
            summary.files.insert(path, cov);
        }
    }
    summary
}

/// Parses an `LCOV` tracefile string into a deterministic [`LcovSummary`] without panicking.
#[must_use]
pub fn parse_lcov(input: &str) -> LcovSummary {
    let mut current_file: Option<String> = None;
    let mut current_acc = FileAccumulator::default();
    let mut files_acc: BTreeMap<String, FileAccumulator> = BTreeMap::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line == "end_of_record" {
            let flushed = core::mem::take(&mut current_acc);
            if let Some(path) = current_file.take() {
                files_acc.entry(path).or_default().merge_from(flushed);
            }
        } else if let Some(sf) = line.strip_prefix("SF:") {
            current_file = Some(normalize_sf_path(sf));
        } else {
            apply_record_directive(line, &mut current_acc);
        }
    }
    if let Some(path) = current_file.take() {
        files_acc.entry(path).or_default().merge_from(current_acc);
    }
    finalize_lcov_summary(files_acc)
}
