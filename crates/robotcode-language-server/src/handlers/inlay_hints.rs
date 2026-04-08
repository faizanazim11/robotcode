//! `textDocument/inlayHint` handler.
//!
//! Returns inlay hints showing argument names for keyword calls within a range.
//!
//! Example: `My Keyword    value1    value2`
//! becomes: `My Keyword    arg1: value1    arg2: value2`

use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section};
use robotcode_robot::diagnostics::{keyword_finder::KeywordMatch, Namespace};

use super::utils::position_in_range;

/// Compute inlay hints for `file` within `range`.
pub fn inlay_hints(file: &File, ns: &Namespace, range: Range) -> Vec<InlayHint> {
    let mut hints: Vec<InlayHint> = Vec::new();

    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    collect_body_hints(&tc.body, ns, file, range, &mut hints);
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    collect_body_hints(&task.body, ns, file, range, &mut hints);
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    collect_body_hints(&kw.body, ns, file, range, &mut hints);
                }
            }
            _ => {}
        }
    }

    hints
}

fn collect_body_hints(
    items: &[BodyItem],
    ns: &Namespace,
    file: &File,
    range: Range,
    out: &mut Vec<InlayHint>,
) {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                let call_pos = Position {
                    line: kc.position.line,
                    character: kc.position.column,
                };
                if !position_in_range(call_pos, range) {
                    continue;
                }

                // Find keyword argument names.
                let arg_names = resolve_arg_names(ns, file, &kc.name);

                // Emit a hint for each positional argument that has a name.
                for (i, arg) in kc.args.iter().enumerate() {
                    if let Some(name) = arg_names.get(i) {
                        // Skip if the argument already looks like `name=value`.
                        if arg.contains('=') {
                            continue;
                        }
                        // Position hint before the argument value, on the same line.
                        // The exact character offset is approximate.
                        let arg_col = kc.position.column
                            + kc.name.len() as u32
                            + (i as u32 + 1) * 4  // 4 spaces per separator (approx)
                            + kc.args[..i].iter().map(|a| a.len() as u32 + 4).sum::<u32>();

                        // Strip variable sigil and braces from the parameter name.
                        // E.g. "${name}" → "name", "@{my_list}" → "my_list".
                        let display_name = name
                            .trim_start_matches(['$', '@', '&', '%'])
                            .trim_matches(['{', '}']);

                        out.push(InlayHint {
                            position: Position {
                                line: kc.position.line,
                                character: arg_col,
                            },
                            label: InlayHintLabel::String(format!("{}:", display_name)),
                            kind: Some(InlayHintKind::PARAMETER),
                            text_edits: None,
                            tooltip: None,
                            padding_left: Some(false),
                            padding_right: Some(true),
                            data: None,
                        });
                    }
                }
            }
            BodyItem::For(f) => {
                collect_body_hints(&f.body, ns, file, range, out);
            }
            BodyItem::While(w) => {
                collect_body_hints(&w.body, ns, file, range, out);
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    collect_body_hints(&branch.body, ns, file, range, out);
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    collect_body_hints(&branch.body, ns, file, range, out);
                }
            }
            _ => {}
        }
    }
}

fn resolve_arg_names(ns: &Namespace, file: &File, kw_name: &str) -> Vec<String> {
    // Try namespace keywords first.
    if let KeywordMatch::Found(kw) = ns.find_keyword(kw_name) {
        return kw.args.iter().map(|a| a.name.clone()).collect();
    }

    // Try local keywords.
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            for kw in &s.body {
                if kw.name == kw_name {
                    return kw
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
                }
            }
        }
    }

    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    fn full_range() -> Range {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 9999,
                character: 9999,
            },
        }
    }

    #[test]
    fn test_inlay_hints_for_local_keyword() {
        let src = "*** Keywords ***\nMy Keyword\n    [Arguments]    ${name}    ${value}\n    Log    ${name}\n*** Test Cases ***\nMy Test\n    My Keyword    hello    world\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let hints = inlay_hints(&file, &ns, full_range());
        // Should emit hints for "name:" and "value:" arguments.
        assert!(!hints.is_empty(), "Should have at least one inlay hint");
        let labels: Vec<String> = hints
            .iter()
            .filter_map(|h| {
                if let InlayHintLabel::String(s) = &h.label {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            labels.iter().any(|l| l.contains("name:")),
            "Should hint 'name:'"
        );
    }

    #[test]
    fn test_no_hints_for_named_args() {
        let src = "*** Keywords ***\nMy Keyword\n    [Arguments]    ${name}\n    Log    ${name}\n*** Test Cases ***\nMy Test\n    My Keyword    name=hello\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let hints = inlay_hints(&file, &ns, full_range());
        // "name=hello" already has explicit name, no hint needed.
        assert!(hints.is_empty());
    }
}
