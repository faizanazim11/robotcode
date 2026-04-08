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

/// Return the start column (0-indexed character offset) of each
/// Robot Framework separator-delimited token on `line`.
///
/// RF treats 2+ consecutive spaces or a tab character as a token separator.
/// Leading whitespace is skipped (it counts as indentation, not as a token).
pub fn token_cols(line: &str) -> Vec<u32> {
    let bytes = line.as_bytes();
    let mut cols = Vec::new();
    let mut i = 0;

    // Skip leading indentation.
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() {
        return cols;
    }

    let mut token_start = i;
    while i < bytes.len() {
        if bytes[i] == b'\t' || (bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1] == b' ') {
            // End of token — record its start column.
            if i > token_start {
                cols.push(token_start as u32);
            }
            // Skip all separator characters.
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            token_start = i;
        } else {
            i += 1;
        }
    }
    if token_start < bytes.len() {
        cols.push(token_start as u32);
    }
    cols
}

/// Split `text` into lines (by `'\n'`) and return a `Vec<&str>`.
///
/// Unlike `str::lines()`, this preserves trailing empty lines so that line
/// indices directly correspond to LSP 0-based line numbers.
pub fn text_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
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
