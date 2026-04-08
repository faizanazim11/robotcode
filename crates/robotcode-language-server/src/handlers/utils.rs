//! Shared utilities for LSP feature handlers.

use lsp_types::{Position, Range};
use robotcode_rf_parser::parser::ast;

/// Convert an AST [`ast::Position`] to an LSP [`Range`].
pub fn ast_pos_to_range(p: &ast::Position) -> Range {
    Range {
        start: Position {
            line: p.line,
            character: p.column,
        },
        end: Position {
            line: p.end_line,
            character: p.end_column,
        },
    }
}

/// Return `true` if the LSP `pos` falls within `range` (inclusive).
pub fn position_in_range(pos: Position, range: Range) -> bool {
    if pos.line < range.start.line || pos.line > range.end.line {
        return false;
    }
    if pos.line == range.start.line && pos.character < range.start.character {
        return false;
    }
    if pos.line == range.end.line && pos.character > range.end.character {
        return false;
    }
    true
}

/// Return `true` if the LSP `pos` falls within the AST [`ast::Position`].
pub fn position_in_ast(pos: Position, p: &ast::Position) -> bool {
    position_in_range(pos, ast_pos_to_range(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_in_range() {
        let r = Range {
            start: Position {
                line: 1,
                character: 4,
            },
            end: Position {
                line: 1,
                character: 12,
            },
        };
        assert!(position_in_range(
            Position {
                line: 1,
                character: 6
            },
            r
        ));
        assert!(!position_in_range(
            Position {
                line: 0,
                character: 6
            },
            r
        ));
        assert!(!position_in_range(
            Position {
                line: 1,
                character: 3
            },
            r
        ));
        assert!(!position_in_range(
            Position {
                line: 1,
                character: 13
            },
            r
        ));
    }
}
