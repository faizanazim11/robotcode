//! `textDocument/foldingRange` handler.
//!
//! Returns folding ranges for:
//! - Sections (Settings, Variables, Test Cases, Keywords, etc.)
//! - Individual test cases and keywords
//! - Control flow blocks: FOR, WHILE, IF branches, TRY branches

use lsp_types::{FoldingRange, FoldingRangeKind};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, SettingItem, VariableItem};

/// Compute folding ranges for `file`.
pub fn folding_ranges(file: &File) -> Vec<FoldingRange> {
    let mut ranges: Vec<FoldingRange> = Vec::new();

    for section in &file.sections {
        match section {
            Section::Settings(s) => {
                // Fold the whole section if it has any body items.
                if !s.body.is_empty() {
                    let start_line = s.header.position.line;
                    let end_line = last_setting_line(&s.body);
                    if end_line > start_line {
                        ranges.push(region(start_line, end_line));
                    }
                }
            }

            Section::Variables(s) => {
                if !s.body.is_empty() {
                    let start_line = s.header.position.line;
                    let end_line = last_variable_item_line(&s.body);
                    if end_line > start_line {
                        ranges.push(region(start_line, end_line));
                    }
                }
            }

            Section::TestCases(s) => {
                if !s.body.is_empty() {
                    let last_tc = s.body.last().unwrap();
                    let section_end =
                        last_body_line(&last_tc.body).unwrap_or(last_tc.position.line);
                    if section_end > s.header.position.line {
                        ranges.push(region(s.header.position.line, section_end));
                    }
                }
                for tc in &s.body {
                    if let Some(end) = last_body_line(&tc.body) {
                        if end > tc.position.line {
                            ranges.push(region(tc.position.line, end));
                        }
                    }
                    collect_body_ranges(&tc.body, &mut ranges);
                }
            }

            Section::Tasks(s) => {
                if !s.body.is_empty() {
                    let last_task = s.body.last().unwrap();
                    let section_end =
                        last_body_line(&last_task.body).unwrap_or(last_task.position.line);
                    if section_end > s.header.position.line {
                        ranges.push(region(s.header.position.line, section_end));
                    }
                }
                for task in &s.body {
                    if let Some(end) = last_body_line(&task.body) {
                        if end > task.position.line {
                            ranges.push(region(task.position.line, end));
                        }
                    }
                    collect_body_ranges(&task.body, &mut ranges);
                }
            }

            Section::Keywords(s) => {
                if !s.body.is_empty() {
                    let last_kw = s.body.last().unwrap();
                    let section_end =
                        last_body_line(&last_kw.body).unwrap_or(last_kw.position.line);
                    if section_end > s.header.position.line {
                        ranges.push(region(s.header.position.line, section_end));
                    }
                }
                for kw in &s.body {
                    if let Some(end) = last_body_line(&kw.body) {
                        if end > kw.position.line {
                            ranges.push(region(kw.position.line, end));
                        }
                    }
                    collect_body_ranges(&kw.body, &mut ranges);
                }
            }

            Section::Comments(s) => {
                if !s.body.is_empty() {
                    let end = s.body.last().map(|c| c.position.end_line).unwrap_or(0);
                    if end > s.header.position.line {
                        ranges.push(FoldingRange {
                            start_line: s.header.position.line,
                            end_line: end,
                            kind: Some(FoldingRangeKind::Comment),
                            start_character: None,
                            end_character: None,
                            collapsed_text: None,
                        });
                    }
                }
            }

            Section::Invalid(_) => {}
        }
    }

    ranges
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn region(start_line: u32, end_line: u32) -> FoldingRange {
    FoldingRange {
        start_line,
        end_line,
        kind: Some(FoldingRangeKind::Region),
        start_character: None,
        end_character: None,
        collapsed_text: None,
    }
}

fn last_setting_line(items: &[SettingItem]) -> u32 {
    items
        .iter()
        .map(|item| {
            let pos = match item {
                SettingItem::LibraryImport(x) => &x.position,
                SettingItem::ResourceImport(x) => &x.position,
                SettingItem::VariablesImport(x) => &x.position,
                SettingItem::Documentation(x) => &x.position,
                SettingItem::Metadata(x) => &x.position,
                SettingItem::SuiteSetup(x) => &x.position,
                SettingItem::SuiteTeardown(x) => &x.position,
                SettingItem::TestSetup(x) => &x.position,
                SettingItem::TestTeardown(x) => &x.position,
                SettingItem::TestTemplate(x) => &x.position,
                SettingItem::TestTags(x) => &x.position,
                SettingItem::DefaultTags(x) => &x.position,
                SettingItem::ForceTags(x) => &x.position,
                SettingItem::KeywordTags(x) => &x.position,
                SettingItem::TaskTags(x) => &x.position,
                SettingItem::Comment(x) => &x.position,
                SettingItem::EmptyLine(x) => &x.position,
                SettingItem::Error(x) => &x.position,
            };
            pos.end_line
        })
        .max()
        .unwrap_or(0)
}

fn last_variable_item_line(items: &[VariableItem]) -> u32 {
    items
        .iter()
        .map(|item| match item {
            VariableItem::Variable(x) => x.position.end_line,
            VariableItem::Comment(x) => x.position.end_line,
            VariableItem::EmptyLine(x) => x.position.end_line,
            VariableItem::Error(x) => x.position.end_line,
        })
        .max()
        .unwrap_or(0)
}

fn last_body_line(items: &[BodyItem]) -> Option<u32> {
    items.iter().map(body_item_end_line).max()
}

fn body_item_end_line(item: &BodyItem) -> u32 {
    match item {
        BodyItem::Documentation(x) => x.position.end_line,
        BodyItem::Arguments(x) => x.position.end_line,
        BodyItem::Tags(x) => x.position.end_line,
        BodyItem::Setup(x) => x.position.end_line,
        BodyItem::Teardown(x) => x.position.end_line,
        BodyItem::Template(x) => x.position.end_line,
        BodyItem::Timeout(x) => x.position.end_line,
        BodyItem::ReturnSetting(x) => x.position.end_line,
        BodyItem::KeywordCall(x) => x.position.end_line,
        BodyItem::TemplateArguments(x) => x.position.end_line,
        BodyItem::For(x) => last_body_line(&x.body).unwrap_or(x.position.end_line),
        BodyItem::While(x) => last_body_line(&x.body).unwrap_or(x.position.end_line),
        BodyItem::If(x) => x
            .branches
            .iter()
            .flat_map(|b| b.body.iter().map(body_item_end_line))
            .max()
            .unwrap_or(x.position.end_line),
        BodyItem::Try(x) => x
            .branches
            .iter()
            .flat_map(|b| b.body.iter().map(body_item_end_line))
            .max()
            .unwrap_or(x.position.end_line),
        BodyItem::Break(x) => x.position.end_line,
        BodyItem::Continue(x) => x.position.end_line,
        BodyItem::Return(x) => x.position.end_line,
        BodyItem::Comment(x) => x.position.end_line,
        BodyItem::EmptyLine(x) => x.position.end_line,
        BodyItem::Error(x) => x.position.end_line,
    }
}

fn collect_body_ranges(items: &[BodyItem], out: &mut Vec<FoldingRange>) {
    for item in items {
        match item {
            BodyItem::For(f) => {
                if let Some(end) = last_body_line(&f.body) {
                    if end > f.position.line {
                        out.push(region(f.position.line, end));
                    }
                }
                collect_body_ranges(&f.body, out);
            }
            BodyItem::While(w) => {
                if let Some(end) = last_body_line(&w.body) {
                    if end > w.position.line {
                        out.push(region(w.position.line, end));
                    }
                }
                collect_body_ranges(&w.body, out);
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    if let Some(end) = last_body_line(&branch.body) {
                        if end > branch.position.line {
                            out.push(region(branch.position.line, end));
                        }
                    }
                    collect_body_ranges(&branch.body, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    if let Some(end) = last_body_line(&branch.body) {
                        if end > branch.position.line {
                            out.push(region(branch.position.line, end));
                        }
                    }
                    collect_body_ranges(&branch.body, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::utils::ast_pos_to_range;
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_section_folding() {
        let src = "*** Settings ***\nLibrary    Collections\n\n*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let ranges = folding_ranges(&file);
        // Should have at least one range for Keywords section
        assert!(!ranges.is_empty());
    }

    #[test]
    fn test_for_loop_folding() {
        let src = "*** Test Cases ***\nMy Test\n    FOR    ${i}    IN RANGE    3\n        Log    ${i}\n    END\n";
        let file = parse(src);
        let ranges = folding_ranges(&file);
        // Should fold the FOR block
        assert!(!ranges.is_empty());
    }

    #[test]
    fn test_if_block_folding() {
        let src = "*** Test Cases ***\nMy Test\n    IF    True\n        Log    yes\n    END\n";
        let file = parse(src);
        let ranges = folding_ranges(&file);
        assert!(!ranges.is_empty());
    }

    #[test]
    fn test_range_start_before_end() {
        let src = "*** Keywords ***\nLong Keyword\n    Step 1\n    Step 2\n    Step 3\n";
        let file = parse(src);
        let ranges = folding_ranges(&file);
        for r in &ranges {
            assert!(r.start_line <= r.end_line, "start must be <= end");
        }
    }

    #[test]
    fn test_empty_file_no_ranges() {
        let file = parse("");
        let ranges = folding_ranges(&file);
        assert!(ranges.is_empty());
    }

    fn pos_to_ast(p: &lsp_types::Position) -> String {
        format!("{}:{}", p.line, p.character)
    }

    #[test]
    fn test_range_uses_ast_pos_to_range() {
        use robotcode_rf_parser::lexer::tokens::Position;
        let p = Position {
            line: 3,
            column: 0,
            end_line: 5,
            end_column: 10,
        };
        let r = ast_pos_to_range(&p);
        assert_eq!(pos_to_ast(&r.start), "3:0");
        assert_eq!(pos_to_ast(&r.end), "5:10");
    }
}
