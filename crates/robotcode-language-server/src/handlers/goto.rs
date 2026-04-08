//! `textDocument/definition` handler.
//!
//! Resolves the definition location for:
//! - Keyword calls → keyword definition site in the same file or (future) imported resources
//! - Variable references → variable declaration site in the same file

use lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, VariableItem};
use robotcode_robot::diagnostics::Namespace;

use super::utils::{ast_pos_to_range, position_in_ast};

/// Find the definition location for the token at `pos`.
pub fn goto_definition(
    file: &File,
    ns: &Namespace,
    uri: &Url,
    pos: Position,
) -> Option<GotoDefinitionResponse> {
    // Determine what's at the cursor.
    if let Some(token) = token_at_position(file, pos) {
        match token {
            Token::Keyword(name) => {
                // Look in local keywords first.
                for section in &file.sections {
                    if let Section::Keywords(s) = section {
                        for kw in &s.body {
                            if kw.name == name {
                                let range = ast_pos_to_range(&kw.position);
                                return Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: uri.clone(),
                                    range,
                                }));
                            }
                        }
                    }
                }
                // Try the namespace (libraries / resources).
                let kw_match = ns.find_keyword(&name);
                if let robotcode_robot::diagnostics::keyword_finder::KeywordMatch::Found(kw) =
                    kw_match
                {
                    if let Some(source) = &kw.source {
                        if let Ok(def_uri) = Url::from_file_path(source) {
                            let line = kw.line_no.unwrap_or(1).saturating_sub(1);
                            return Some(GotoDefinitionResponse::Scalar(Location {
                                uri: def_uri,
                                range: Range {
                                    start: Position { line, character: 0 },
                                    end: Position { line, character: 0 },
                                },
                            }));
                        }
                    }
                }
                None
            }
            Token::Variable(normalized) => {
                // Look in the Variables section.
                for section in &file.sections {
                    if let Section::Variables(s) = section {
                        for item in &s.body {
                            if let VariableItem::Variable(v) = item {
                                if normalize_var_name(&v.name) == normalized {
                                    return Some(GotoDefinitionResponse::Scalar(Location {
                                        uri: uri.clone(),
                                        range: ast_pos_to_range(&v.position),
                                    }));
                                }
                            }
                        }
                    }
                }
                // Also look in keyword [Arguments] definitions.
                for section in &file.sections {
                    if let Section::Keywords(s) = section {
                        for kw in &s.body {
                            for item in &kw.body {
                                if let BodyItem::Arguments(args) = item {
                                    for arg in &args.args {
                                        if normalize_var_name(arg) == normalized {
                                            return Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: uri.clone(),
                                                    range: ast_pos_to_range(&args.position),
                                                },
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                None
            }
        }
    } else {
        None
    }
}

// ── Token identification ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Token {
    Keyword(String),
    Variable(String),
}

fn token_at_position(file: &File, pos: Position) -> Option<Token> {
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
                    // Check if cursor is on the keyword name.
                    let name_col = kc.position.column;
                    let name_end = name_col + kc.name.len() as u32;
                    if pos.character >= name_col && pos.character <= name_end {
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

fn normalize_var_name(name: &str) -> String {
    let inner = name.trim_start_matches(['$', '@', '&', '%']);
    let inner = inner.trim_matches(['{', '}']);
    inner.to_lowercase().replace(' ', "_").replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;
    use robotcode_robot::diagnostics::Namespace;

    fn test_uri() -> Url {
        Url::parse("file:///test.robot").unwrap()
    }

    #[test]
    fn test_goto_local_keyword_definition() {
        let src = "*** Test Cases ***\nMy Test\n    My Keyword\n*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 2,
            character: 4,
        }; // on "My Keyword" call
        let result = goto_definition(&file, &ns, &test_uri(), pos);
        assert!(result.is_some(), "Should find local keyword definition");
        if let Some(GotoDefinitionResponse::Scalar(loc)) = result {
            // The definition is at line 4 (0-indexed)
            assert_eq!(loc.range.start.line, 4);
        }
    }

    #[test]
    fn test_goto_no_definition_for_section_header() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 0,
            character: 0,
        }; // on section header
        let result = goto_definition(&file, &ns, &test_uri(), pos);
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_var_name() {
        assert_eq!(normalize_var_name("${MY_VAR}"), "my_var");
        assert_eq!(normalize_var_name("@{List Items}"), "list_items");
    }
}
