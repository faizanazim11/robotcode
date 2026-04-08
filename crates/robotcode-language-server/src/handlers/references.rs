//! `textDocument/references` handler.
//!
//! Returns all locations where a keyword or variable is used within the
//! current file (workspace-wide references require a cross-file index,
//! which will be added in a future iteration).

use lsp_types::{Location, Position, Range, Url};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, VariableItem};
use robotcode_rf_parser::variables::search_variable;

use super::utils::{ast_pos_to_range, position_in_ast, text_lines, token_cols};

/// Find all references to the symbol at `pos` in `file`.
///
/// `text` is the raw document source used to compute accurate argument column offsets.
pub fn references(
    file: &File,
    text: &str,
    uri: &Url,
    pos: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let lines = text_lines(text);
    let token = token_at(file, pos);
    match token {
        Some(Token::Keyword(name)) => keyword_references(file, uri, &name, include_declaration),
        Some(Token::Variable(norm)) => {
            variable_references(file, &lines, uri, &norm, include_declaration)
        }
        None => vec![],
    }
}

// ── Token identification ──────────────────────────────────────────────────────

#[derive(Debug)]
enum Token {
    Keyword(String),
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

// ── Keyword references ────────────────────────────────────────────────────────

fn keyword_references(
    file: &File,
    uri: &Url,
    name: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locs = Vec::new();

    for section in &file.sections {
        match section {
            Section::Keywords(s) => {
                for kw in &s.body {
                    if include_declaration && kw.name == name {
                        locs.push(Location {
                            uri: uri.clone(),
                            range: ast_pos_to_range(&kw.position),
                        });
                    }
                    collect_keyword_calls_in_body(&kw.body, name, uri, &mut locs);
                }
            }
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_keyword_calls_in_body(&tc.body, name, uri, &mut locs);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_keyword_calls_in_body(&task.body, name, uri, &mut locs);
                }
            }
            _ => {}
        }
    }

    locs
}

fn collect_keyword_calls_in_body(
    items: &[BodyItem],
    name: &str,
    uri: &Url,
    out: &mut Vec<Location>,
) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.name == name {
                    out.push(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: kc.position.line,
                                character: kc.position.column,
                            },
                            end: Position {
                                line: kc.position.line,
                                character: kc.position.column + kc.name.len() as u32,
                            },
                        },
                    });
                }
            }
            BodyItem::For(f) => collect_keyword_calls_in_body(&f.body, name, uri, out),
            BodyItem::While(w) => collect_keyword_calls_in_body(&w.body, name, uri, out),
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_keyword_calls_in_body(&branch.body, name, uri, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_keyword_calls_in_body(&branch.body, name, uri, out);
                }
            }
            _ => {}
        }
    }
}

// ── Variable references ───────────────────────────────────────────────────────

fn variable_references(
    file: &File,
    lines: &[&str],
    uri: &Url,
    norm_name: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locs = Vec::new();

    for section in &file.sections {
        match section {
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if normalize_var_name(&v.name) == norm_name {
                            if include_declaration {
                                locs.push(Location {
                                    uri: uri.clone(),
                                    range: ast_pos_to_range(&v.position),
                                });
                            }
                        }
                    }
                }
            }
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_var_refs_in_body(&tc.body, lines, norm_name, uri, &mut locs);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_var_refs_in_body(&task.body, lines, norm_name, uri, &mut locs);
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    collect_var_refs_in_body(&kw.body, lines, norm_name, uri, &mut locs);
                }
            }
            _ => {}
        }
    }

    locs
}

fn collect_var_refs_in_body(
    items: &[BodyItem],
    lines: &[&str],
    norm_name: &str,
    uri: &Url,
    out: &mut Vec<Location>,
) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                let line_text = lines.get(kc.position.line as usize).copied().unwrap_or("");
                let cols = token_cols(line_text);
                let name_idx = kc.assigns.len();
                for (i, arg) in kc.args.iter().enumerate() {
                    let base_col = cols.get(name_idx + 1 + i).copied().unwrap_or(0);
                    collect_var_refs_in_text(arg, norm_name, kc.position.line, base_col, uri, out);
                }
            }
            BodyItem::For(f) => collect_var_refs_in_body(&f.body, lines, norm_name, uri, out),
            BodyItem::While(w) => collect_var_refs_in_body(&w.body, lines, norm_name, uri, out),
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_var_refs_in_body(&branch.body, lines, norm_name, uri, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_var_refs_in_body(&branch.body, lines, norm_name, uri, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_var_refs_in_text(
    text: &str,
    norm_name: &str,
    line: u32,
    base_col: u32,
    uri: &Url,
    out: &mut Vec<Location>,
) {
    let mut remaining = text;
    let mut offset: u32 = 0;
    while let Some(m) = search_variable(remaining) {
        let var_text = &remaining[m.start..m.end];
        if normalize_var_name(var_text) == norm_name {
            out.push(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line,
                        character: base_col + offset + m.start as u32,
                    },
                    end: Position {
                        line,
                        character: base_col + offset + m.end as u32,
                    },
                },
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
    fn test_keyword_references_includes_call_sites() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n*** Test Cases ***\nMy Test\n    My Keyword\n    My Keyword\n";
        let file = parse(src);
        let pos = Position {
            line: 1,
            character: 0,
        }; // On keyword definition
        let refs = references(&file, src, &test_uri(), pos, false);
        // Should find 2 call sites.
        assert_eq!(refs.len(), 2, "Should find 2 call sites of My Keyword");
    }

    #[test]
    fn test_keyword_references_with_declaration() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n*** Test Cases ***\nMy Test\n    My Keyword\n";
        let file = parse(src);
        let pos = Position {
            line: 1,
            character: 0,
        };
        let refs = references(&file, src, &test_uri(), pos, true);
        assert_eq!(refs.len(), 2, "Should find definition + 1 call site");
    }

    #[test]
    fn test_no_references_on_empty_pos() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let pos = Position {
            line: 0,
            character: 0,
        }; // Section header
        let refs = references(&file, src, &test_uri(), pos, false);
        assert!(refs.is_empty());
    }
}
