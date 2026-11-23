//! Source spans, severities, and structured diagnostics with AI remediation hints.

use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::path::RepoPath;

/// Validation error when constructing a 1-indexed [`Span`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpanError {
    /// Line numbers are 1-indexed and must not be `0`.
    ZeroLine,
    /// `end_line` was strictly smaller than `start_line`.
    InvertedRange {
        /// Provided starting line.
        start_line: u32,
        /// Provided ending line.
        end_line: u32,
    },
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ZeroLine => f.write_str("span lines are 1-indexed and cannot be zero"),
            Self::InvertedRange {
                start_line,
                end_line,
            } => write!(f, "invalid span range {start_line}..={end_line}"),
        }
    }
}

impl core::error::Error for SpanError {}

/// Inclusive 1-indexed line range within a source file.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Span {
    start_line: u32,
    end_line: u32,
}

impl Span {
    /// Canonical fallback span covering line 1 (`1..=1`).
    pub const FIRST: Self = Self {
        start_line: 1,
        end_line: 1,
    };

    /// Creates a validated inclusive line range (`start_line..=end_line`).
    ///
    /// # Errors
    ///
    /// Returns [`SpanError`] if either bound is `0` or `end_line < start_line`.
    pub const fn new(start_line: u32, end_line: u32) -> Result<Self, SpanError> {
        if start_line == 0 || end_line == 0 {
            return Err(SpanError::ZeroLine);
        }
        if end_line < start_line {
            return Err(SpanError::InvertedRange {
                start_line,
                end_line,
            });
        }
        Ok(Self {
            start_line,
            end_line,
        })
    }

    /// Returns the 1-indexed starting line.
    #[must_use]
    pub const fn start_line(self) -> u32 {
        self.start_line
    }

    /// Returns the 1-indexed ending line.
    #[must_use]
    pub const fn end_line(self) -> u32 {
        self.end_line
    }

    /// Returns the inclusive number of lines covered by this span.
    #[must_use]
    pub const fn line_count(self) -> u32 {
        self.end_line
            .saturating_sub(self.start_line)
            .saturating_add(1)
    }
}

/// Diagnostic severity level.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Severity {
    /// Informational note that does not fail checks.
    Info,
    /// Actionable warning.
    Warning,
    /// Blocking architectural or quality violation.
    Error,
}

/// Pure static-analysis diagnostic emitted by a rule or runner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable rule identifier (e.g. `"GF001"`).
    pub rule_id: String,
    /// Violation severity.
    pub severity: Severity,
    /// Human- and agent-actionable explanation.
    pub message: String,
    /// Explicit architectural refactoring guidance preventing AI anti-patterns.
    pub remediation: String,
    /// Concrete metric value measured on the offending construct.
    pub measured: u32,
    /// Configured upper threshold violated by `measured`.
    pub threshold: u32,
    /// Target repository-relative file path.
    pub path: RepoPath,
    /// Source span of the violation.
    pub span: Span,
}

impl Ord for Diagnostic {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.path, self.span, &self.rule_id, &self.message).cmp(&(
            &other.path,
            other.span,
            &other.rule_id,
            &other.message,
        ))
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::string::ToString;

    use super::{Diagnostic, RepoPath, Severity, Span, SpanError};

    #[test]
    fn span_and_diagnostic_ordering_are_deterministic() -> Result<(), Box<dyn core::error::Error>> {
        let span_a = Span::new(10, 24)?;
        let span_b = Span::new(2, 5)?;
        assert_eq!(span_a.line_count(), 15);
        assert_eq!(Span::new(0, 1), Err(SpanError::ZeroLine));
        assert_ne!(SpanError::ZeroLine.to_string(), "");
        assert_ne!(
            SpanError::InvertedRange {
                start_line: 8,
                end_line: 3
            }
            .to_string(),
            ""
        );

        let d1 = Diagnostic {
            rule_id: "GF002".to_string(),
            severity: Severity::Error,
            message: "msg".to_string(),
            remediation: "extract helper".to_string(),
            measured: 60,
            threshold: 50,
            path: RepoPath::new("src/b.rs")?,
            span: span_a,
        };
        let d2 = Diagnostic {
            path: RepoPath::new("src/a.rs")?,
            span: span_b,
            ..d1.clone()
        };
        assert!(d2 < d1);
        Ok(())
    }
}
