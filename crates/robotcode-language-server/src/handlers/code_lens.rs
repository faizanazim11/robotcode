//! `textDocument/codeLens` handler.
//!
//! Emits code lenses on test cases and tasks:
//! - **Run Test** — triggers the test runner for the focused test
//! - **Debug Test** — launches the debugger for the focused test

use lsp_types::{CodeLens, Command, Position, Range, Url};
use robotcode_rf_parser::parser::ast::{File, Section};

use super::utils::ast_pos_to_range;

/// Compute code lenses for `file`.
pub fn code_lens(file: &File, uri: &Url) -> Vec<CodeLens> {
    let mut lenses: Vec<CodeLens> = Vec::new();

    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    let range = ast_pos_to_range(&tc.position);
                    let test_name = tc.name.clone();

                    lenses.push(run_lens(uri, &test_name, range));
                    lenses.push(debug_lens(uri, &test_name, range));
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    let range = ast_pos_to_range(&task.position);
                    let task_name = task.name.clone();

                    lenses.push(run_lens(uri, &task_name, range));
                    lenses.push(debug_lens(uri, &task_name, range));
                }
            }
            _ => {}
        }
    }

    lenses
}

fn run_lens(uri: &Url, name: &str, range: Range) -> CodeLens {
    CodeLens {
        range: Range {
            start: Position {
                line: range.start.line,
                character: 0,
            },
            end: Position {
                line: range.start.line,
                character: 0,
            },
        },
        command: Some(Command {
            title: "▶ Run Test".to_string(),
            command: "robotcode.runTest".to_string(),
            arguments: Some(vec![
                serde_json::json!(uri.as_str()),
                serde_json::json!(name),
            ]),
        }),
        data: None,
    }
}

fn debug_lens(uri: &Url, name: &str, range: Range) -> CodeLens {
    CodeLens {
        range: Range {
            start: Position {
                line: range.start.line,
                character: 0,
            },
            end: Position {
                line: range.start.line,
                character: 0,
            },
        },
        command: Some(Command {
            title: "🐛 Debug Test".to_string(),
            command: "robotcode.debugTest".to_string(),
            arguments: Some(vec![
                serde_json::json!(uri.as_str()),
                serde_json::json!(name),
            ]),
        }),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    fn test_uri() -> Url {
        Url::parse("file:///test.robot").unwrap()
    }

    #[test]
    fn test_code_lens_for_test_cases() {
        let src =
            "*** Test Cases ***\nMy First Test\n    Log    hi\nMy Second Test\n    Log    bye\n";
        let file = parse(src);
        let lenses = code_lens(&file, &test_uri());
        // 2 tests × 2 lenses each = 4.
        assert_eq!(lenses.len(), 4);
    }

    #[test]
    fn test_code_lens_has_correct_commands() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n";
        let file = parse(src);
        let lenses = code_lens(&file, &test_uri());
        let commands: Vec<&str> = lenses
            .iter()
            .filter_map(|l| l.command.as_ref())
            .map(|c| c.command.as_str())
            .collect();
        assert!(commands.contains(&"robotcode.runTest"));
        assert!(commands.contains(&"robotcode.debugTest"));
    }

    #[test]
    fn test_no_lens_for_keywords_only() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let lenses = code_lens(&file, &test_uri());
        assert!(lenses.is_empty(), "Keywords do not get code lenses");
    }

    #[test]
    fn test_lens_includes_test_name_in_args() {
        let src = "*** Test Cases ***\nSuite Login Test\n    Log    hi\n";
        let file = parse(src);
        let lenses = code_lens(&file, &test_uri());
        let run_lens = lenses.iter().find(|l| {
            l.command
                .as_ref()
                .map(|c| c.command == "robotcode.runTest")
                .unwrap_or(false)
        });
        assert!(run_lens.is_some());
        let args = run_lens
            .unwrap()
            .command
            .as_ref()
            .unwrap()
            .arguments
            .as_ref()
            .unwrap();
        assert!(args[1].as_str().unwrap() == "Suite Login Test");
    }
}
