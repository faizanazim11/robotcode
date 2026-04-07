//! LSP protocol type re-exports and custom extensions.
//!
//! Re-exports from the `lsp-types` crate plus project-specific additions.

pub use lsp_types::*;

/// A document URI string as used by the LSP protocol.
pub type DocumentUri = String;

/// Extension trait for [`Position`] comparisons (already implemented by lsp-types,
/// but re-exposed here for convenience).
pub trait PositionExt {
    /// Return `true` if `self` comes before `other` in document order.
    fn is_before(&self, other: &Self) -> bool;
    /// Return `true` if `self` comes after `other` in document order.
    fn is_after(&self, other: &Self) -> bool;
}

impl PositionExt for Position {
    fn is_before(&self, other: &Self) -> bool {
        self.line < other.line || (self.line == other.line && self.character < other.character)
    }

    fn is_after(&self, other: &Self) -> bool {
        self.line > other.line || (self.line == other.line && self.character > other.character)
    }
}

/// Extension trait for [`Range`].
pub trait RangeExt {
    /// Return `true` if `self` contains the given position.
    fn contains(&self, pos: &Position) -> bool;
    /// Return `true` if `self` and `other` overlap.
    fn overlaps(&self, other: &Range) -> bool;
}

impl RangeExt for Range {
    fn contains(&self, pos: &Position) -> bool {
        // LSP ranges are half-open: [start, end).  The end position is exclusive.
        (pos.line > self.start.line
            || (pos.line == self.start.line && pos.character >= self.start.character))
            && (pos.line < self.end.line
                || (pos.line == self.end.line && pos.character < self.end.character))
    }

    fn overlaps(&self, other: &Range) -> bool {
        // Two half-open ranges [a,b) and [c,d) overlap iff a < d && c < b.
        self.start < other.end && other.start < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_is_before() {
        let a = Position {
            line: 0,
            character: 5,
        };
        let b = Position {
            line: 1,
            character: 0,
        };
        assert!(a.is_before(&b));
        assert!(!b.is_before(&a));
    }

    #[test]
    fn test_range_contains() {
        let range = Range {
            start: Position {
                line: 1,
                character: 2,
            },
            end: Position {
                line: 3,
                character: 4,
            },
        };
        assert!(range.contains(&Position {
            line: 2,
            character: 0
        }));
        assert!(!range.contains(&Position {
            line: 0,
            character: 0
        }));
    }
}
