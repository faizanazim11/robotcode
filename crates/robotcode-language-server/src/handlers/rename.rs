//! `textDocument/rename` handler.
//!
//! Renames keywords and variables within the current file.
//! Workspace-wide rename will be added when a cross-file index is implemented.

use lsp_types::{Position, TextEdit, Url, WorkspaceEdit};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, VariableItem};
use robotcode_rf_parser::variables::search_variable;
use std::collections::HashMap;

use super::utils::{ast_pos_to_range, position_in_ast};

/// Rename the symbol at `pos` to `new_name`.
pub fn rename(file: &File, uri: &Url, pos: Position, new_name: String) -> Option<WorkspaceEdit> {
    let token = token_at(file, pos)?;
    let edits = match &token {
        Token::Keyword(name) => keyword_rename_edits(file, name, &new_name),
        Token::Variable(norm) => variable_rename_edits(file, norm, &new_name),
    };

    if edits.is_empty() {
        return None;
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

// ── Token identification ──────────────────────────────────────────────────────

#[derive(Debug)]
enum Token {
    /// Keyword name as written in the source.
    Keyword(String),
    /// Normalized variable name (lowercase, underscores).
    Variable(String),
}

fn token_at(file: &File, pos: Position) -> Option<Token> {
    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    if let Some(t) = find_in_body(&tc.body, pos) {
                        return Some(t);
                    }
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    if let Some(t) = find_in_body(&task.body, pos) {
                        return Some(t);
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if position_in_ast(pos, &kw.position) && pos.line == kw.position.line {
                        return Some(Token::Keyword(kw.name.clone()));
                    }
                    if let Some(t) = find_in_body(&kw.body, pos) {
                        return Some(t);
                    }
                }
            }
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if position_in_ast(pos, &v.position) {
                            return Some(Token::Variable(normalize_var_name(&v.name)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_body(items: &[BodyItem], pos: Position) -> Option<Token> {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.position.line == pos.line {
                    let name_end = kc.position.column + kc.name.len() as u32;
                    if pos.character >= kc.position.column && pos.character <= name_end {
                        return Some(Token::Keyword(kc.name.clone()));
                    }
                }
            }
            BodyItem::For(f) => {
                if let Some(t) = find_in_body(&f.body, pos) {
                    return Some(t);
                }
            }
            BodyItem::While(w) => {
                if let Some(t) = find_in_body(&w.body, pos) {
                    return Some(t);
                }
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    if let Some(t) = find_in_body(&branch.body, pos) {
                        return Some(t);
                    }
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    if let Some(t) = find_in_body(&branch.body, pos) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

// ── Keyword rename ────────────────────────────────────────────────────────────

fn keyword_rename_edits(file: &File, old_name: &str, new_name: &str) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    for section in &file.sections {
        match section {
            Section::Keywords(s) => {
                for kw in &s.body {
                    if kw.name == old_name {
                        // Rename the definition.
                        let range = lsp_types::Range {
                            start: lsp_types::Position { line: kw.position.line, character: kw.position.column },
                            end: lsp_types::Position { line: kw.position.line, character: kw.position.column + kw.name.len() as u32 },
                        };
                        edits.push(TextEdit { range, new_text: new_name.to_string() });
                    }
                    collect_kw_call_edits(&kw.body, old_name, new_name, &mut edits);
                }
            }
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_kw_call_edits(&tc.body, old_name, new_name, &mut edits);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_kw_call_edits(&task.body, old_name, new_name, &mut edits);
                }
            }
            _ => {}
        }
    }

    edits
}

fn collect_kw_call_edits(items: &[BodyItem], old_name: &str, new_name: &str, out: &mut Vec<TextEdit>) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.name == old_name {
                    let range = lsp_types::Range {
                        start: lsp_types::Position { line: kc.position.line, character: kc.position.column },
                        end: lsp_types::Position { line: kc.position.line, character: kc.position.column + kc.name.len() as u32 },
                    };
                    out.push(TextEdit { range, new_text: new_name.to_string() });
                }
            }
            BodyItem::For(f) => collect_kw_call_edits(&f.body, old_name, new_name, out),
            BodyItem::While(w) => collect_kw_call_edits(&w.body, old_name, new_name, out),
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_kw_call_edits(&branch.body, old_name, new_name, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_kw_call_edits(&branch.body, old_name, new_name, out);
                }
            }
            _ => {}
        }
    }
}

// ── Variable rename ───────────────────────────────────────────────────────────

fn variable_rename_edits(file: &File, norm_old: &str, new_name: &str) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    for section in &file.sections {
        match section {
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if normalize_var_name(&v.name) == norm_old {
                            // Build the new variable name preserving the sigil.
                            let sigil = v.name.chars().next().unwrap_or('$');
                            let new_var_name = format!("{}{{{}}}", sigil, new_name.trim_matches(['{', '}', '$', '@', '&', '%']));
                            let range = ast_pos_to_range(&v.position);
                            let name_range = lsp_types::Range {
                                start: range.start,
                                end: lsp_types::Position {
                                    line: range.start.line,
                                    character: range.start.character + v.name.len() as u32,
                                },
                            };
                            edits.push(TextEdit { range: name_range, new_text: new_var_name });
                        }
                    }
                }
            }
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_var_edits_in_body(&tc.body, norm_old, new_name, &mut edits);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_var_edits_in_body(&task.body, norm_old, new_name, &mut edits);
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    collect_var_edits_in_body(&kw.body, norm_old, new_name, &mut edits);
                }
            }
            _ => {}
        }
    }

    edits
}

fn collect_var_edits_in_body(items: &[BodyItem], norm_old: &str, new_name: &str, out: &mut Vec<TextEdit>) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                for arg in &kc.args {
                    collect_var_text_edits(arg, norm_old, new_name, kc.position.line, out);
                }
            }
            BodyItem::For(f) => collect_var_edits_in_body(&f.body, norm_old, new_name, out),
            BodyItem::While(w) => collect_var_edits_in_body(&w.body, norm_old, new_name, out),
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_var_edits_in_body(&branch.body, norm_old, new_name, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_var_edits_in_body(&branch.body, norm_old, new_name, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_var_text_edits(text: &str, norm_old: &str, new_name: &str, line: u32, out: &mut Vec<TextEdit>) {
    let mut remaining = text;
    let mut offset: u32 = 0;
    while let Some(m) = search_variable(remaining) {
        let var_text = &remaining[m.start..m.end];
        if normalize_var_name(var_text) == norm_old {
            // Preserve the sigil.
            let sigil = var_text.chars().next().unwrap_or('$');
            let new_var = format!("{}{{{}}}", sigil, new_name.trim_matches(['{', '}', '$', '@', '&', '%']));
            out.push(TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position { line, character: offset + m.start as u32 },
                    end: lsp_types::Position { line, character: offset + m.end as u32 },
                },
                new_text: new_var,
            });
        }
        if m.end >= remaining.len() {
            break;
        }
        offset += m.end as u32;
        remaining = &remaining[m.end..];
    }
}

fn normalize_var_name(name: &str) -> String {
    let inner = name.trim_start_matches(['$', '@', '&', '%']);
    let inner = inner.trim_matches(['{', '}']);
    inner.to_lowercase().replace(' ', "_").replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    fn test_uri() -> Url {
        Url::parse("file:///test.robot").unwrap()
    }

    #[test]
    fn test_rename_keyword_definition_and_calls() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n*** Test Cases ***\nMy Test\n    My Keyword\n    My Keyword\n";
        let file = parse(src);
        let pos = Position { line: 1, character: 0 };
        let result = rename(&file, &test_uri(), pos, "My New Keyword".to_string());
        assert!(result.is_some());
        let edit = result.unwrap();
        let edits = edit.changes.unwrap().remove(&test_uri()).unwrap();
        // Should have 3 edits: 1 definition + 2 call sites.
        assert_eq!(edits.len(), 3);
        for e in &edits {
            assert_eq!(e.new_text, "My New Keyword");
        }
    }

    #[test]
    fn test_rename_nothing_on_section_header() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let pos = Position { line: 0, character: 0 };
        let result = rename(&file, &test_uri(), pos, "Irrelevant".to_string());
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_var_name() {
        assert_eq!(normalize_var_name("${MY_VAR}"), "my_var");
        assert_eq!(normalize_var_name("@{My List}"), "my_list");
    }
}
