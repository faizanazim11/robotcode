//! `textDocument/documentHighlight` handler.
//!
//! Highlights all occurrences of the keyword or variable under the cursor
//! within the current document.

use lsp_types::{DocumentHighlight, DocumentHighlightKind, Position, Range};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, VariableItem};
use robotcode_rf_parser::variables::search_variable;

use super::utils::{ast_pos_to_range, position_in_ast};

/// Find highlights for the token at `pos` in `file`.
pub fn document_highlight(file: &File, pos: Position) -> Vec<DocumentHighlight> {
    // 1. Identify what is at the cursor position.
    if let Some(token) = find_token_at(file, pos) {
        // 2. Find all occurrences of that token.
        collect_occurrences(file, &token)
    } else {
        vec![]
    }
}

// ── Token at cursor ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum TokenKind {
    Keyword(String),
    Variable(String),
}

fn find_token_at(file: &File, pos: Position) -> Option<TokenKind> {
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
                    // Check if cursor is on the keyword name itself.
                    if position_in_ast(pos, &kw.position) && pos.line == kw.position.line {
                        return Some(TokenKind::Keyword(kw.name.clone()));
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
                            return Some(TokenKind::Variable(normalize_var_name(&v.name)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_body(items: &[BodyItem], pos: Position) -> Option<TokenKind> {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if position_in_ast(pos, &kc.position) && pos.line == kc.position.line {
                    // Check if cursor is on the keyword name.
                    let name_end_col = kc.position.column + kc.name.len() as u32;
                    if pos.character >= kc.position.column && pos.character <= name_end_col {
                        return Some(TokenKind::Keyword(kc.name.clone()));
                    }
                    // Check args for variable references.
                    for arg in &kc.args {
                        if let Some(var) = var_at_position(arg, pos) {
                            return Some(TokenKind::Variable(var));
                        }
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

fn var_at_position(text: &str, pos: Position) -> Option<String> {
    let mut remaining = text;
    while let Some(m) = search_variable(remaining) {
        let start_char = m.start as u32;
        let end_char = m.end as u32;
        if pos.character >= start_char && pos.character <= end_char {
            let var_text = &text[m.start..m.end];
            return Some(normalize_var_name(var_text));
        }
        if m.end >= remaining.len() {
            break;
        }
        remaining = &remaining[m.end..];
    }
    None
}

fn normalize_var_name(name: &str) -> String {
    // Strip leading sigil and braces: `${MY_VAR}` → `my_var`
    let inner = name.trim_start_matches(['$', '@', '&', '%']);
    let inner = inner.trim_matches(['{', '}']);
    inner.to_lowercase().replace(' ', "_").replace('-', "_")
}

// ── Occurrence collection ─────────────────────────────────────────────────────

fn collect_occurrences(file: &File, token: &TokenKind) -> Vec<DocumentHighlight> {
    let mut highlights = Vec::new();

    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_body_occurrences(&tc.body, token, &mut highlights);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_body_occurrences(&task.body, token, &mut highlights);
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if let TokenKind::Keyword(name) = token {
                        if kw.name == *name {
                            highlights.push(DocumentHighlight {
                                range: ast_pos_to_range(&kw.position),
                                kind: Some(DocumentHighlightKind::TEXT),
                            });
                        }
                    }
                    collect_body_occurrences(&kw.body, token, &mut highlights);
                }
            }
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if let TokenKind::Variable(name) = token {
                            if normalize_var_name(&v.name) == *name {
                                highlights.push(DocumentHighlight {
                                    range: ast_pos_to_range(&v.position),
                                    kind: Some(DocumentHighlightKind::TEXT),
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    highlights
}

fn collect_body_occurrences(items: &[BodyItem], token: &TokenKind, out: &mut Vec<DocumentHighlight>) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => match token {
                TokenKind::Keyword(name) => {
                    if kc.name == *name {
                        out.push(DocumentHighlight {
                            range: Range {
                                start: Position { line: kc.position.line, character: kc.position.column },
                                end: Position { line: kc.position.line, character: kc.position.column + kc.name.len() as u32 },
                            },
                            kind: Some(DocumentHighlightKind::READ),
                        });
                    }
                }
                TokenKind::Variable(norm) => {
                    for arg in &kc.args {
                        collect_var_refs_in_text(arg, norm, kc.position.line, out);
                    }
                }
            },
            BodyItem::For(f) => {
                collect_body_occurrences(&f.body, token, out);
            }
            BodyItem::While(w) => {
                collect_body_occurrences(&w.body, token, out);
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_body_occurrences(&branch.body, token, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_body_occurrences(&branch.body, token, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_var_refs_in_text(text: &str, norm_name: &str, line: u32, out: &mut Vec<DocumentHighlight>) {
    let mut remaining = text;
    let mut offset: u32 = 0;
    while let Some(m) = search_variable(remaining) {
        let var_text = &remaining[m.start..m.end];
        if normalize_var_name(var_text) == norm_name {
            out.push(DocumentHighlight {
                range: Range {
                    start: Position { line, character: offset + m.start as u32 },
                    end: Position { line, character: offset + m.end as u32 },
                },
                kind: Some(DocumentHighlightKind::READ),
            });
        }
        if m.end >= remaining.len() {
            break;
        }
        offset += m.end as u32;
        remaining = &remaining[m.end..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_keyword_highlight() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hello\n*** Test Cases ***\nMy Test\n    My Keyword\n";
        let file = parse(src);
        // Position on the keyword definition line.
        let pos = Position { line: 1, character: 0 };
        let highlights = document_highlight(&file, pos);
        // Should highlight the definition and the call.
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_no_highlight_on_empty_area() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hello\n";
        let file = parse(src);
        let pos = Position { line: 0, character: 0 };
        let highlights = document_highlight(&file, pos);
        // Section header is not a keyword or variable.
        assert!(highlights.is_empty());
    }

    #[test]
    fn test_normalize_var_name() {
        assert_eq!(normalize_var_name("${MY_VAR}"), "my_var");
        assert_eq!(normalize_var_name("@{my list}"), "my_list");
        assert_eq!(normalize_var_name("${X}"), "x");
    }
}
