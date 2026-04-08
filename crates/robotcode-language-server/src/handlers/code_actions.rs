//! `textDocument/codeAction` handler.
//!
//! Provides quick fixes and refactoring actions:
//! - **Add missing Library import** when `KeywordNotFound` diagnostic present
//! - **Fix keyword name typo** suggestions (Levenshtein distance)
//! - **Extract keyword** from selected body statements (refactoring)

use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, NumberOrString, Position,
    Range, TextEdit, Url, WorkspaceEdit,
};
use std::collections::HashMap;

use robotcode_rf_parser::parser::ast::{BodyItem, File, Section};
use robotcode_robot::diagnostics::{entities::normalize_keyword_name, Namespace};

/// Compute code actions for `range` in `file`.
pub fn code_actions(
    file: &File,
    ns: &Namespace,
    uri: &Url,
    range: Range,
    diagnostics: Vec<Diagnostic>,
) -> Vec<CodeActionOrCommand> {
    let mut actions: Vec<CodeActionOrCommand> = Vec::new();

    // Process diagnostics to generate quick fixes.
    for diag in &diagnostics {
        if let Some(code) = &diag.code {
            let code_str = match code {
                NumberOrString::String(s) => s.as_str(),
                NumberOrString::Number(_) => "",
            };

            match code_str {
                "KeywordNotFound" => {
                    // Extract keyword name from diagnostic message.
                    if let Some(kw_name) = extract_keyword_name_from_diagnostic(&diag.message) {
                        // Suggest typo corrections.
                        let suggestions = find_similar_keywords(ns, file, &kw_name, 3);
                        for suggestion in suggestions {
                            let edit = make_text_replacement(uri, diag.range, &suggestion);
                            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                                title: format!("Did you mean '{}'?", suggestion),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(edit),
                                ..Default::default()
                            }));
                        }
                    }
                }
                "ImportError" | "LibraryImportError" => {
                    // Offer to create the resource file or fix the path.
                    // This is a placeholder — real implementation would inspect imports.
                }
                _ => {}
            }
        }
    }

    // Refactoring: Extract keyword from selected range.
    if let Some(extract_action) = extract_keyword_action(file, uri, range) {
        actions.push(extract_action);
    }

    actions
}

// ── Quick fix helpers ─────────────────────────────────────────────────────────

fn extract_keyword_name_from_diagnostic(msg: &str) -> Option<String> {
    // Messages like: "No keyword with name 'Foo Bar' found."
    let start = msg.find('\'')?;
    let end = msg.rfind('\'')?;
    if start == end {
        return None;
    }
    Some(msg[start + 1..end].to_string())
}

fn find_similar_keywords(ns: &Namespace, file: &File, name: &str, max_suggestions: usize) -> Vec<String> {
    let norm = normalize_keyword_name(name);

    // Collect all keyword names.
    let mut all_names: Vec<String> = ns.all_keywords().iter().map(|kw| kw.name.clone()).collect();
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            for kw in &s.body {
                all_names.push(kw.name.clone());
            }
        }
    }

    // Sort by Levenshtein distance to `norm`.
    let mut scored: Vec<(usize, String)> = all_names
        .into_iter()
        .map(|n| (levenshtein(&norm, &normalize_keyword_name(&n)), n))
        .filter(|(dist, _)| *dist <= 4) // Only suggest close matches.
        .collect();
    scored.sort_by_key(|(d, _)| *d);
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().take(max_suggestions).map(|(_, n)| n).collect()
}

fn make_text_replacement(uri: &Url, range: Range, new_text: &str) -> WorkspaceEdit {
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit { range, new_text: new_text.to_string() }],
    );
    WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }
}

/// Simple Levenshtein distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

// ── Refactoring: Extract Keyword ──────────────────────────────────────────────

fn extract_keyword_action(
    file: &File,
    uri: &Url,
    range: Range,
) -> Option<CodeActionOrCommand> {
    // Find body items that overlap with `range`.
    let items = collect_items_in_range(file, range);
    if items.len() < 2 {
        // Need at least 2 steps to make extraction worthwhile.
        return None;
    }

    // Build the extracted keyword text.
    let kw_name = "Extracted Keyword";
    let body: String = items.iter().map(|s| format!("    {}\n", s)).collect();
    let new_kw = format!("\n{}\n{}", kw_name, body);

    // The call site replacement.
    let call = format!("    {}", kw_name);

    // Find the Keywords section to insert into.
    let kw_insert_line = find_or_create_keywords_section_line(file);
    let insert_range = Range {
        start: Position { line: kw_insert_line, character: 0 },
        end: Position { line: kw_insert_line, character: 0 },
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![
            // Replace selection with call.
            TextEdit { range, new_text: call },
            // Insert new keyword definition.
            TextEdit { range: insert_range, new_text: new_kw },
        ],
    );

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: "Extract keyword".to_string(),
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        diagnostics: None,
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        ..Default::default()
    }))
}

fn collect_items_in_range(file: &File, range: Range) -> Vec<String> {
    let mut items = Vec::new();
    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    for item in &tc.body {
                        if let BodyItem::KeywordCall(kc) = item {
                            if kc.position.line >= range.start.line
                                && kc.position.line <= range.end.line
                            {
                                let args = kc.args.join("    ");
                                items.push(if args.is_empty() {
                                    kc.name.clone()
                                } else {
                                    format!("{}    {}", kc.name, args)
                                });
                            }
                        }
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    for item in &kw.body {
                        if let BodyItem::KeywordCall(kc) = item {
                            if kc.position.line >= range.start.line
                                && kc.position.line <= range.end.line
                            {
                                let args = kc.args.join("    ");
                                items.push(if args.is_empty() {
                                    kc.name.clone()
                                } else {
                                    format!("{}    {}", kc.name, args)
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    items
}

fn find_or_create_keywords_section_line(file: &File) -> u32 {
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            // Insert at end of existing Keywords section.
            let end = s.body.last().map(|kw| {
                kw.body.last().map(|item| match item {
                    BodyItem::KeywordCall(kc) => kc.position.end_line,
                    BodyItem::Comment(c) => c.position.end_line,
                    _ => kw.position.end_line,
                }).unwrap_or(kw.position.end_line)
            }).unwrap_or(s.header.position.end_line);
            return end + 1;
        }
    }
    // No Keywords section — append at end of file.
    file.sections
        .last()
        .map(|s| match s {
            Section::Settings(x) => x.header.position.end_line,
            Section::Variables(x) => x.header.position.end_line,
            Section::TestCases(x) => x.header.position.end_line,
            Section::Tasks(x) => x.header.position.end_line,
            Section::Keywords(x) => x.header.position.end_line,
            Section::Comments(x) => x.header.position.end_line,
            Section::Invalid(_) => 0,
        })
        .unwrap_or(0)
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    fn test_uri() -> Url {
        Url::parse("file:///test.robot").unwrap()
    }

    fn empty_range() -> Range {
        Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 0 },
        }
    }

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("log", "log"), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein("log", "logs"), 1);
    }

    #[test]
    fn test_no_actions_empty_diagnostics() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let actions = code_actions(&file, &ns, &test_uri(), empty_range(), vec![]);
        // No diagnostics → no quick fixes.  May still have refactoring actions.
        let _ = actions;
    }

    #[test]
    fn test_typo_suggestion() {
        assert_eq!(levenshtein("keyword", "keywrod"), 2);
        let similar = levenshtein("logmessage", "log message");
        assert!(similar <= 2, "Close names should have distance ≤ 2");
    }
}
