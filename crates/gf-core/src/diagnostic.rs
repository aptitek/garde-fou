//! Source spans, severities, and structured diagnostics.

use alloc::string::String;
use core::fmt;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    start_line: u32,
    end_line: u32,
}

impl Span {
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    /// Informational note that does not fail checks.
    Info,
    /// Actionable warning.
    Warning,
    /// Blocking architectural or quality violation.
    Error,
}

/// Pure static-analysis diagnostic emitted by a rule or runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Stable rule identifier (e.g. `"GF001"`).
    pub rule_id: String,
    /// Violation severity.
    pub severity: Severity,
    /// Human- and agent-actionable explanation.
    pub message: String,
    /// Target repository-relative file path.
    pub path: RepoPath,
    /// Source span of the violation.
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::{Diagnostic, RepoPath, Severity, Span, SpanError};

    #[test]
    fn span_computes_inclusive_line_counts() -> Result<(), SpanError> {
        let span = Span::new(10, 24)?;
        assert_eq!(span.start_line(), 10);
        assert_eq!(span.end_line(), 24);
        assert_eq!(span.line_count(), 15);
        Ok(())
    }

    #[test]
    fn span_rejects_zero_and_inverted_ranges() -> Result<(), crate::path::RepoPathError> {
        assert_eq!(Span::new(0, 1), Err(SpanError::ZeroLine));
        assert_eq!(
            Span::new(8, 3),
            Err(SpanError::InvertedRange {
                start_line: 8,
                end_line: 3,
            })
        );
        assert_ne!(SpanError::ZeroLine.to_string(), "");
        assert_ne!(
            SpanError::InvertedRange {
                start_line: 8,
                end_line: 3
            }
            .to_string(),
            ""
        );

        let path = RepoPath::new("src/main.rs")?;
        if let Ok(span) = Span::new(1, 1) {
            let diag = Diagnostic {
                rule_id: "GF001".to_string(),
                severity: Severity::Error,
                message: "too long".to_string(),
                path,
                span,
            };
            assert_eq!(diag.severity, Severity::Error);
        }
        Ok(())
    }
}
