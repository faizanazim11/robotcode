//! `textDocument/selectionRange` handler.
//!
//! Returns nested `SelectionRange` values that allow the editor to expand
//! the selection stepwise through the syntax tree:
//! - Token → Statement line → Block (test/keyword) → Section → File

use lsp_types::{Position, Range, SelectionRange};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, VariableItem};

use super::utils::{ast_pos_to_range, position_in_ast};

/// Compute selection ranges for each of `positions` in `file`.
pub fn selection_ranges(file: &File, positions: Vec<Position>) -> Vec<SelectionRange> {
    positions
        .into_iter()
        .map(|pos| selection_range_for(file, pos))
        .collect()
}

/// Build the nested `SelectionRange` chain for a single position.
fn selection_range_for(file: &File, pos: Position) -> SelectionRange {
    // Walk the AST to find the tightest range that contains `pos`.
    for section in &file.sections {
        match section {
            Section::Settings(s) => {
                if position_in_ast(pos, &s.header.position) || pos.line >= s.header.position.line {
                    return SelectionRange {
                        range: ast_pos_to_range(&s.header.position),
                        parent: None,
                    };
                }
            }
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if position_in_ast(pos, &v.position) {
                            let token_range = ast_pos_to_range(&v.position);
                            let section_range = ast_pos_to_range(&s.header.position);
                            return SelectionRange {
                                range: token_range,
                                parent: Some(Box::new(SelectionRange {
                                    range: section_range,
                                    parent: None,
                                })),
                            };
                        }
                    }
                }
            }
            Section::TestCases(s) => {
                for tc in &s.body {
                    if pos.line == tc.position.line {
                        return SelectionRange {
                            range: ast_pos_to_range(&tc.position),
                            parent: Some(Box::new(SelectionRange {
                                range: ast_pos_to_range(&s.header.position),
                                parent: None,
                            })),
                        };
                    }
                    if let Some(body_range) = find_in_body(&tc.body, pos) {
                        let tc_range = ast_pos_to_range(&tc.position);
                        let section_range = ast_pos_to_range(&s.header.position);
                        return SelectionRange {
                            range: body_range,
                            parent: Some(Box::new(SelectionRange {
                                range: tc_range,
                                parent: Some(Box::new(SelectionRange {
                                    range: section_range,
                                    parent: None,
                                })),
                            })),
                        };
                    }
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    if pos.line == task.position.line {
                        return SelectionRange {
                            range: ast_pos_to_range(&task.position),
                            parent: Some(Box::new(SelectionRange {
                                range: ast_pos_to_range(&s.header.position),
                                parent: None,
                            })),
                        };
                    }
                    if let Some(body_range) = find_in_body(&task.body, pos) {
                        let task_range = ast_pos_to_range(&task.position);
                        let section_range = ast_pos_to_range(&s.header.position);
                        return SelectionRange {
                            range: body_range,
                            parent: Some(Box::new(SelectionRange {
                                range: task_range,
                                parent: Some(Box::new(SelectionRange {
                                    range: section_range,
                                    parent: None,
                                })),
                            })),
                        };
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if pos.line == kw.position.line {
                        return SelectionRange {
                            range: ast_pos_to_range(&kw.position),
                            parent: Some(Box::new(SelectionRange {
                                range: ast_pos_to_range(&s.header.position),
                                parent: None,
                            })),
                        };
                    }
                    if let Some(body_range) = find_in_body(&kw.body, pos) {
                        let kw_range = ast_pos_to_range(&kw.position);
                        let section_range = ast_pos_to_range(&s.header.position);
                        return SelectionRange {
                            range: body_range,
                            parent: Some(Box::new(SelectionRange {
                                range: kw_range,
                                parent: Some(Box::new(SelectionRange {
                                    range: section_range,
                                    parent: None,
                                })),
                            })),
                        };
                    }
                }
            }
            _ => {}
        }
    }

    // Fallback: return a zero-width range at the position.
    SelectionRange {
        range: Range {
            start: pos,
            end: pos,
        },
        parent: None,
    }
}

fn find_in_body(items: &[BodyItem], pos: Position) -> Option<Range> {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.position.line == pos.line {
                    return Some(ast_pos_to_range(&kc.position));
                }
            }
            BodyItem::For(f) => {
                if pos.line == f.position.line {
                    return Some(ast_pos_to_range(&f.position));
                }
                if let Some(r) = find_in_body(&f.body, pos) {
                    return Some(r);
                }
            }
            BodyItem::While(w) => {
                if pos.line == w.position.line {
                    return Some(ast_pos_to_range(&w.position));
                }
                if let Some(r) = find_in_body(&w.body, pos) {
                    return Some(r);
                }
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    if pos.line == branch.position.line {
                        return Some(ast_pos_to_range(&branch.position));
                    }
                    if let Some(r) = find_in_body(&branch.body, pos) {
                        return Some(r);
                    }
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    if pos.line == branch.position.line {
                        return Some(ast_pos_to_range(&branch.position));
                    }
                    if let Some(r) = find_in_body(&branch.body, pos) {
                        return Some(r);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_selection_range_on_keyword_call() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n";
        let file = parse(src);
        let pos = Position { line: 2, character: 4 };
        let ranges = selection_ranges(&file, vec![pos]);
        assert_eq!(ranges.len(), 1);
        // Should have at least one parent (the test case).
        assert!(ranges[0].parent.is_some(), "Keyword call should have a parent range");
    }

    #[test]
    fn test_selection_range_on_block_name() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n";
        let file = parse(src);
        let pos = Position { line: 1, character: 0 };
        let ranges = selection_ranges(&file, vec![pos]);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range.start.line, 1);
    }

    #[test]
    fn test_selection_range_fallback_on_unknown_pos() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let pos = Position { line: 999, character: 0 }; // Outside file
        let ranges = selection_ranges(&file, vec![pos]);
        assert_eq!(ranges.len(), 1);
        // Should return a fallback zero-width range at the position.
        assert_eq!(ranges[0].range.start, pos);
    }
}
