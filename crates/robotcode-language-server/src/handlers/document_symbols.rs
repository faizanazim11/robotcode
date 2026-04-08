//! `textDocument/documentSymbol` handler.
//!
//! Returns a hierarchy of symbols for a Robot Framework file:
//! - Test cases and tasks → `SymbolKind::Function`
//! - Keywords → `SymbolKind::Method`
//! - Variable declarations → `SymbolKind::Variable`
//! - Sections → `SymbolKind::Namespace`

use lsp_types::{DocumentSymbol, Range, SymbolKind};
use robotcode_rf_parser::parser::ast::{File, Section, SettingItem, VariableItem};

use super::utils::ast_pos_to_range;

#[allow(deprecated)]
/// Compute document symbols for `file`.
pub fn document_symbols(file: &File) -> Vec<DocumentSymbol> {
    let mut symbols: Vec<DocumentSymbol> = Vec::new();

    for section in &file.sections {
        match section {
            Section::Settings(s) => {
                let section_range = ast_pos_to_range(&s.header.position);
                let mut children: Vec<DocumentSymbol> = Vec::new();

                for item in &s.body {
                    match item {
                        SettingItem::LibraryImport(li) => {
                            children.push(make_symbol(
                                &li.name,
                                SymbolKind::MODULE,
                                ast_pos_to_range(&li.position),
                            ));
                        }
                        SettingItem::ResourceImport(ri) => {
                            children.push(make_symbol(
                                &ri.path,
                                SymbolKind::FILE,
                                ast_pos_to_range(&ri.position),
                            ));
                        }
                        SettingItem::VariablesImport(vi) => {
                            children.push(make_symbol(
                                &vi.path,
                                SymbolKind::FILE,
                                ast_pos_to_range(&vi.position),
                            ));
                        }
                        _ => {}
                    }
                }

                symbols.push(DocumentSymbol {
                    name: s.header.name.clone(),
                    kind: SymbolKind::NAMESPACE,
                    range: section_range,
                    selection_range: section_range,
                    detail: None,
                    tags: None,
                    deprecated: None,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }

            Section::Variables(s) => {
                let mut children: Vec<DocumentSymbol> = Vec::new();

                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        children.push(make_symbol(
                            &v.name,
                            SymbolKind::VARIABLE,
                            ast_pos_to_range(&v.position),
                        ));
                    }
                }

                if !children.is_empty() || !s.body.is_empty() {
                    let section_range = ast_pos_to_range(&s.header.position);
                    symbols.push(DocumentSymbol {
                        name: s.header.name.clone(),
                        kind: SymbolKind::NAMESPACE,
                        range: section_range,
                        selection_range: section_range,
                        detail: None,
                        tags: None,
                        deprecated: None,
                        children: if children.is_empty() {
                            None
                        } else {
                            Some(children)
                        },
                    });
                }
            }

            Section::TestCases(s) => {
                let mut children: Vec<DocumentSymbol> = Vec::new();

                for tc in &s.body {
                    children.push(make_symbol(
                        &tc.name,
                        SymbolKind::FUNCTION,
                        ast_pos_to_range(&tc.position),
                    ));
                }

                let section_range = ast_pos_to_range(&s.header.position);
                symbols.push(DocumentSymbol {
                    name: s.header.name.clone(),
                    kind: SymbolKind::NAMESPACE,
                    range: section_range,
                    selection_range: section_range,
                    detail: None,
                    tags: None,
                    deprecated: None,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }

            Section::Tasks(s) => {
                let mut children: Vec<DocumentSymbol> = Vec::new();

                for task in &s.body {
                    children.push(make_symbol(
                        &task.name,
                        SymbolKind::FUNCTION,
                        ast_pos_to_range(&task.position),
                    ));
                }

                let section_range = ast_pos_to_range(&s.header.position);
                symbols.push(DocumentSymbol {
                    name: s.header.name.clone(),
                    kind: SymbolKind::NAMESPACE,
                    range: section_range,
                    selection_range: section_range,
                    detail: None,
                    tags: None,
                    deprecated: None,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }

            Section::Keywords(s) => {
                let mut children: Vec<DocumentSymbol> = Vec::new();

                for kw in &s.body {
                    children.push(make_symbol(
                        &kw.name,
                        SymbolKind::METHOD,
                        ast_pos_to_range(&kw.position),
                    ));
                }

                let section_range = ast_pos_to_range(&s.header.position);
                symbols.push(DocumentSymbol {
                    name: s.header.name.clone(),
                    kind: SymbolKind::NAMESPACE,
                    range: section_range,
                    selection_range: section_range,
                    detail: None,
                    tags: None,
                    deprecated: None,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }

            Section::Comments(_) | Section::Invalid(_) => {}
        }
    }

    symbols
}

#[allow(deprecated)]
fn make_symbol(name: &str, kind: SymbolKind, range: Range) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_owned(),
        kind,
        range,
        selection_range: range,
        detail: None,
        tags: None,
        deprecated: None,
        children: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_keywords_appear_as_method_symbols() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hello\n";
        let file = parse(src);
        let symbols = document_symbols(&file);
        assert!(symbols.iter().any(|s| s.name.contains("Keywords")));
        let kw_section = symbols
            .iter()
            .find(|s| s.name.contains("Keywords"))
            .unwrap();
        let children = kw_section.children.as_ref().unwrap();
        assert!(children.iter().any(|s| s.name == "My Keyword"));
    }

    #[test]
    fn test_test_cases_appear_as_function_symbols() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hello\n";
        let file = parse(src);
        let symbols = document_symbols(&file);
        let tc_section = symbols.iter().find(|s| s.name.contains("Test")).unwrap();
        let children = tc_section.children.as_ref().unwrap();
        assert!(children.iter().any(|s| s.name == "My Test"));
    }

    #[test]
    fn test_variables_appear_as_variable_symbols() {
        let src = "*** Variables ***\n${MY_VAR}    value\n";
        let file = parse(src);
        let symbols = document_symbols(&file);
        let var_section = symbols
            .iter()
            .find(|s| s.name.contains("Variable"))
            .unwrap();
        let children = var_section.children.as_ref().unwrap();
        assert!(children.iter().any(|s| s.name == "${MY_VAR}"));
    }
}
