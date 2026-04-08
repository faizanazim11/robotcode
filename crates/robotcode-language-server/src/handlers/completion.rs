//! `textDocument/completion` handler.
//!
//! Provides completions for:
//! - Keywords (from namespace + local keywords)
//! - Variables (from namespace + local variables)
//! - Setting names in the Settings section
//! - BDD prefixes (Given, When, Then, And, But)

use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section, SettingItem, VariableItem};
use robotcode_robot::diagnostics::Namespace;

/// Compute completions at `pos` in `file`.
pub fn completions(file: &File, ns: &Namespace, pos: Position) -> Vec<CompletionItem> {
    let context = determine_context(file, pos);
    match context {
        CompletionContext::Keyword => keyword_completions(file, ns),
        CompletionContext::Variable => variable_completions(file, ns),
        CompletionContext::Setting => setting_completions(),
        CompletionContext::Unknown => {
            // Provide all completions as fallback.
            let mut items = keyword_completions(file, ns);
            items.extend(variable_completions(file, ns));
            items
        }
    }
}

// ── Context detection ─────────────────────────────────────────────────────────

#[allow(dead_code)]
enum CompletionContext {
    Keyword,
    Variable,
    Setting,
    Unknown,
}

fn determine_context(file: &File, pos: Position) -> CompletionContext {
    // Check if we're in a Settings section.
    for section in &file.sections {
        if let Section::Settings(s) = section {
            if pos.line == s.header.position.line {
                return CompletionContext::Setting;
            }
            for item in &s.body {
                let item_line = match item {
                    SettingItem::LibraryImport(x) => x.position.line,
                    SettingItem::ResourceImport(x) => x.position.line,
                    SettingItem::VariablesImport(x) => x.position.line,
                    SettingItem::Documentation(x) => x.position.line,
                    SettingItem::Metadata(x) => x.position.line,
                    SettingItem::SuiteSetup(x) => x.position.line,
                    SettingItem::SuiteTeardown(x) => x.position.line,
                    SettingItem::TestSetup(x) => x.position.line,
                    SettingItem::TestTeardown(x) => x.position.line,
                    SettingItem::TestTemplate(x) => x.position.line,
                    SettingItem::TestTags(x) => x.position.line,
                    SettingItem::DefaultTags(x) => x.position.line,
                    SettingItem::ForceTags(x) => x.position.line,
                    SettingItem::KeywordTags(x) => x.position.line,
                    SettingItem::TaskTags(x) => x.position.line,
                    SettingItem::Comment(x) => x.position.line,
                    SettingItem::EmptyLine(x) => x.position.line,
                    SettingItem::Error(x) => x.position.line,
                };
                if item_line == pos.line {
                    return CompletionContext::Setting;
                }
            }
        }
    }

    // Check if the position is inside a keyword body (indented content).
    // Indented lines (character > 0) in test/keyword bodies are keyword calls.
    if pos.character >= 4 {
        return CompletionContext::Keyword;
    }

    CompletionContext::Unknown
}

// ── Keyword completions ───────────────────────────────────────────────────────

fn keyword_completions(file: &File, ns: &Namespace) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    // BDD prefixes.
    for prefix in &["Given ", "When ", "Then ", "And ", "But "] {
        items.push(CompletionItem {
            label: prefix.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            insert_text: Some(prefix.to_string()),
            ..Default::default()
        });
    }

    // Keywords from the namespace.
    for kw in ns.all_keywords() {
        let label = match &kw.library_name {
            Some(lib) => format!("{}.{}", lib, kw.name),
            None => kw.name.clone(),
        };
        let detail = kw.library_name.clone().unwrap_or_default();
        let doc = if kw.doc.is_empty() {
            None
        } else {
            Some(lsp_types::Documentation::MarkupContent(
                lsp_types::MarkupContent {
                    kind: lsp_types::MarkupKind::Markdown,
                    value: kw.doc.clone(),
                },
            ))
        };

        // Build snippet with argument placeholders.
        let snippet = if kw.args.is_empty() {
            kw.name.clone()
        } else {
            let args: Vec<String> = kw
                .args
                .iter()
                .enumerate()
                .map(|(i, a)| format!("${{{}:{}}}", i + 1, a.name))
                .collect();
            format!("{}    {}", kw.name, args.join("    "))
        };

        items.push(CompletionItem {
            label,
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(detail),
            documentation: doc,
            insert_text: Some(snippet),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            deprecated: Some(kw.deprecated.is_some()),
            ..Default::default()
        });
    }

    // Local keywords from the file.
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            for kw in &s.body {
                // Avoid duplicating namespace keywords.
                if !items.iter().any(|i| i.label == kw.name) {
                    // Extract [Arguments] from keyword body.
                    let args: Vec<String> = kw
                        .body
                        .iter()
                        .find_map(|item| {
                            if let BodyItem::Arguments(a) = item {
                                Some(a.args.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    let snippet = if args.is_empty() {
                        kw.name.clone()
                    } else {
                        let placeholders: Vec<String> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| format!("${{{}:{}}}", i + 1, a))
                            .collect();
                        format!("{}    {}", kw.name, placeholders.join("    "))
                    };

                    items.push(CompletionItem {
                        label: kw.name.clone(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some("local keyword".to_string()),
                        insert_text: Some(snippet),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }
            }
        }
    }

    items
}

// ── Variable completions ──────────────────────────────────────────────────────

fn variable_completions(file: &File, ns: &Namespace) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    // Built-in RF variables.
    for name in &[
        "${EMPTY}",
        "${SPACE}",
        "${TRUE}",
        "${FALSE}",
        "${NONE}",
        "${TEST NAME}",
        "${TEST STATUS}",
        "${SUITE NAME}",
        "${SUITE STATUS}",
        "${OUTPUT DIR}",
        "${LOG FILE}",
        "${REPORT FILE}",
        "${PREV TEST NAME}",
        "${PREV TEST STATUS}",
        "${PREV TEST MESSAGE}",
    ] {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("built-in variable".to_string()),
            ..Default::default()
        });
    }

    // Variables from namespace.
    for var in ns.all_suite_variables() {
        if !items.iter().any(|i| i.label == var.name) {
            items.push(CompletionItem {
                label: var.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(format!("{:?} variable", var.scope)),
                ..Default::default()
            });
        }
    }

    // Local variables from the Variables section.
    for section in &file.sections {
        if let Section::Variables(s) = section {
            for item in &s.body {
                if let VariableItem::Variable(v) = item {
                    if !items.iter().any(|i| i.label == v.name) {
                        items.push(CompletionItem {
                            label: v.name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("suite variable".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    items
}

// ── Setting completions ───────────────────────────────────────────────────────

fn setting_completions() -> Vec<CompletionItem> {
    let settings = [
        (
            "Library",
            "Library    ${1:name}    ${2:WITH NAME}    ${3:alias}",
        ),
        ("Resource", "Resource    ${1:path/to/resource.robot}"),
        ("Variables", "Variables    ${1:path/to/variables.yaml}"),
        ("Documentation", "Documentation    ${1:description}"),
        ("Suite Setup", "Suite Setup    ${1:keyword}"),
        ("Suite Teardown", "Suite Teardown    ${1:keyword}"),
        ("Test Setup", "Test Setup    ${1:keyword}"),
        ("Test Teardown", "Test Teardown    ${1:keyword}"),
        ("Test Template", "Test Template    ${1:keyword}"),
        ("Test Tags", "Test Tags    ${1:tag}"),
        ("Default Tags", "Default Tags    ${1:tag}"),
        ("Force Tags", "Force Tags    ${1:tag}"),
        ("Metadata", "Metadata    ${1:name}    ${2:value}"),
    ];

    settings
        .iter()
        .map(|(label, snippet)| CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_completions_include_bdd_prefixes() {
        let src = "*** Test Cases ***\nMy Test\n    ";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = completions(&file, &ns, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"Given "),
            "Should include 'Given ' BDD prefix"
        );
    }

    #[test]
    fn test_completions_include_local_keywords() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n*** Test Cases ***\nMy Test\n    ";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 5,
            character: 4,
        };
        let items = completions(&file, &ns, pos);
        assert!(items.iter().any(|i| i.label == "My Keyword"));
    }

    #[test]
    fn test_variable_completions_include_builtins() {
        let src = "*** Variables ***\n${MY}    val\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 0,
            character: 0,
        };
        let items = completions(&file, &ns, pos);
        assert!(items.iter().any(|i| i.label == "${EMPTY}"));
    }
}
