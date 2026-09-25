//! Source coordinates: [`SourcePosition`] and [`SourceRange`] value types.

use serde::{Deserialize, Serialize};

/// A position in a source file with upstream coordinate semantics: `line`
/// is 1-based, `column` is 0-based (UTF-16 code units where the upstream
/// walker computes them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourcePosition {
    /// 1-based line.
    pub line: u32,
    /// 0-based column.
    pub column: u32,
}

impl SourcePosition {
    /// Construct a position.
    pub fn new(line: u32, column: u32) -> Self {
        SourcePosition { line, column }
    }
}

/// A span with `start` pointing at the first character and `end` at the
/// character after the last one of the span. Both positions use upstream
/// line/column semantics (see [`SourcePosition`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRange {
    /// Start position.
    pub start: SourcePosition,
    /// End position.
    pub end: SourcePosition,
}

impl SourceRange {
    /// Construct a range from its start and end positions.
    pub fn new(start: SourcePosition, end: SourcePosition) -> Self {
        SourceRange { start, end }
    }

    /// The number of covered lines (end line minus start line plus one).
    pub fn line_count(self) -> u32 {
        self.end.line.saturating_sub(self.start.line) + 1
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins one-based lines, zero-based columns, and inclusive range endpoints.
    ///
    /// # Panics
    ///
    /// Panics if a source-position or range assertion fails.
    fn position_and_range_semantics() {
        let start = SourcePosition::new(1, 0);
        let end = SourcePosition::new(3, 5);
        let range = SourceRange::new(start, end);
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.column, 0);
        assert_eq!(range.end.column, 5);
        assert_eq!(range.line_count(), 3);
        assert_eq!(serde_json::to_value(range).unwrap()["start"]["line"], 1);
    }
}
