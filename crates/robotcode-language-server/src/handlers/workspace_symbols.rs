//! `workspace/symbol` handler.
//!
//! Returns all test cases, tasks, and keywords across all open documents as
//! workspace symbols.  When a cross-file index is available (future work),
//! this will scan all files in the workspace.

use lsp_types::{Location, SymbolInformation, SymbolKind, Url};
use robotcode_rf_parser::parser::ast::{File, Section, VariableItem};

use super::utils::ast_pos_to_range;

/// Build workspace symbols from `file` at `uri`, filtered by `query`.
///
/// `query` is a substring that the symbol name must contain (case-insensitive).
/// Pass an empty string to return all symbols.
#[allow(deprecated)]
pub fn workspace_symbols(file: &File, uri: &Url, query: &str) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut symbols: Vec<SymbolInformation> = Vec::new();

    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    if query_lower.is_empty() || tc.name.to_lowercase().contains(&query_lower) {
                        symbols.push(SymbolInformation {
                            name: tc.name.clone(),
                            kind: SymbolKind::FUNCTION,
                            location: Location {
                                uri: uri.clone(),
                                range: ast_pos_to_range(&tc.position),
                            },
                            container_name: Some(s.header.name.clone()),
                            deprecated: None,
                            tags: None,
                        });
                    }
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    if query_lower.is_empty() || task.name.to_lowercase().contains(&query_lower) {
                        symbols.push(SymbolInformation {
                            name: task.name.clone(),
                            kind: SymbolKind::FUNCTION,
                            location: Location {
                                uri: uri.clone(),
                                range: ast_pos_to_range(&task.position),
                            },
                            container_name: Some(s.header.name.clone()),
                            deprecated: None,
                            tags: None,
                        });
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if query_lower.is_empty() || kw.name.to_lowercase().contains(&query_lower) {
                        symbols.push(SymbolInformation {
                            name: kw.name.clone(),
                            kind: SymbolKind::METHOD,
                            location: Location {
                                uri: uri.clone(),
                                range: ast_pos_to_range(&kw.position),
                            },
                            container_name: Some(s.header.name.clone()),
                            deprecated: None,
                            tags: None,
                        });
                    }
                }
            }
            Section::Variables(s) => {
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        if query_lower.is_empty() || v.name.to_lowercase().contains(&query_lower) {
                            symbols.push(SymbolInformation {
                                name: v.name.clone(),
                                kind: SymbolKind::VARIABLE,
                                location: Location {
                                    uri: uri.clone(),
                                    range: ast_pos_to_range(&v.position),
                                },
                                container_name: Some(s.header.name.clone()),
                                deprecated: None,
                                tags: None,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    fn test_uri() -> Url {
        Url::parse("file:///test.robot").unwrap()
    }

    #[test]
    fn test_all_symbols_empty_query() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n*** Keywords ***\nMy Keyword\n    Log    hi\n*** Variables ***\n${MY}    val\n";
        let file = parse(src);
        let symbols = workspace_symbols(&file, &test_uri(), "");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"My Test"));
        assert!(names.contains(&"My Keyword"));
        assert!(names.contains(&"${MY}"));
    }

    #[test]
    fn test_symbols_filtered_by_query() {
        let src = "*** Test Cases ***\nLogin Test\n    Log    hi\nLogout Test\n    Log    bye\nOther Test\n    Log    other\n";
        let file = parse(src);
        let symbols = workspace_symbols(&file, &test_uri(), "login");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Login Test");
    }

    #[test]
    fn test_symbols_correct_kind() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let symbols = workspace_symbols(&file, &test_uri(), "");
        let test_sym = symbols.iter().find(|s| s.name == "My Test").unwrap();
        let kw_sym = symbols.iter().find(|s| s.name == "My Keyword").unwrap();
        assert_eq!(test_sym.kind, SymbolKind::FUNCTION);
        assert_eq!(kw_sym.kind, SymbolKind::METHOD);
    }
}
