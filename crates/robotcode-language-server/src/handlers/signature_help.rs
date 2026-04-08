//! `textDocument/signatureHelp` handler.
//!
//! Returns the signature of the keyword being called at `pos`, with the
//! currently active argument highlighted.

use lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel,
    Position, SignatureHelp, SignatureInformation,
};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section};
use robotcode_robot::diagnostics::{keyword_finder::KeywordMatch, Namespace};

/// Compute signature help at `pos`.
pub fn signature_help(file: &File, ns: &Namespace, pos: Position) -> Option<SignatureHelp> {
    let (kw_name, arg_index) = find_keyword_call_at(file, pos)?;
    let info = build_signature(ns, file, &kw_name, arg_index)?;
    Some(info)
}

// ── Find keyword call ─────────────────────────────────────────────────────────

/// Returns `(keyword_name, active_arg_index)`.
fn find_keyword_call_at(file: &File, pos: Position) -> Option<(String, u32)> {
    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    if let Some(r) = find_in_body(&tc.body, pos) {
                        return Some(r);
                    }
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    if let Some(r) = find_in_body(&task.body, pos) {
                        return Some(r);
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if let Some(r) = find_in_body(&kw.body, pos) {
                        return Some(r);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_body(items: &[BodyItem], pos: Position) -> Option<(String, u32)> {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.position.line == pos.line {
                    // Cursor must be after the keyword name (in the args area).
                    let name_end = kc.position.column + kc.name.len() as u32;
                    if pos.character > name_end {
                        // Determine which argument the cursor is in.
                        // Each argument is separated by 2+ spaces in RF.
                        // We approximate based on the number of args and rough cursor position.
                        let args_col = name_end;
                        let relative = pos.character.saturating_sub(args_col);
                        // Rough heuristic: each arg is ~10 chars wide.
                        let arg_index = kc
                            .args
                            .iter()
                            .enumerate()
                            .find_map(|(i, _a)| {
                                // Try to find which arg column the cursor falls in.
                                let estimated_col = i as u32 * 12;
                                if relative >= estimated_col && (i + 1 >= kc.args.len() || relative < (i as u32 + 1) * 12) {
                                    Some(i as u32)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        return Some((kc.name.clone(), arg_index));
                    }
                }
            }
            BodyItem::For(f) => {
                if let Some(r) = find_in_body(&f.body, pos) {
                    return Some(r);
                }
            }
            BodyItem::While(w) => {
                if let Some(r) = find_in_body(&w.body, pos) {
                    return Some(r);
                }
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    if let Some(r) = find_in_body(&branch.body, pos) {
                        return Some(r);
                    }
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    if let Some(r) = find_in_body(&branch.body, pos) {
                        return Some(r);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

// ── Signature building ────────────────────────────────────────────────────────

fn build_signature(ns: &Namespace, file: &File, kw_name: &str, arg_index: u32) -> Option<SignatureHelp> {
    // Try namespace keywords first.
    if let KeywordMatch::Found(kw) = ns.find_keyword(kw_name) {
        let label = format_signature_label(&kw.name, &kw.args.iter().map(|a| a.name.clone()).collect::<Vec<_>>());
        let parameters: Vec<ParameterInformation> = kw
            .args
            .iter()
            .map(|a| ParameterInformation {
                label: ParameterLabel::Simple(a.name.clone()),
                documentation: None,
            })
            .collect();

        let documentation = if kw.doc.is_empty() {
            None
        } else {
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: kw.doc.clone(),
            }))
        };

        let active_param = if (arg_index as usize) < parameters.len() {
            Some(arg_index)
        } else if !parameters.is_empty() {
            Some(parameters.len() as u32 - 1)
        } else {
            None
        };

        return Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation,
                parameters: Some(parameters),
                active_parameter: active_param,
            }],
            active_signature: Some(0),
            active_parameter: active_param,
        });
    }

    // Try local keywords.
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            for kw in &s.body {
                if kw.name == kw_name {
                    let args: Vec<String> = kw.body.iter().find_map(|item| {
                        if let BodyItem::Arguments(a) = item {
                            Some(a.args.clone())
                        } else {
                            None
                        }
                    }).unwrap_or_default();

                    let label = format_signature_label(&kw.name, &args);
                    let parameters: Vec<ParameterInformation> = args
                        .iter()
                        .map(|a| ParameterInformation {
                            label: ParameterLabel::Simple(a.clone()),
                            documentation: None,
                        })
                        .collect();

                    let active_param = if (arg_index as usize) < parameters.len() {
                        Some(arg_index)
                    } else if !parameters.is_empty() {
                        Some(parameters.len() as u32 - 1)
                    } else {
                        None
                    };

                    return Some(SignatureHelp {
                        signatures: vec![SignatureInformation {
                            label,
                            documentation: None,
                            parameters: Some(parameters),
                            active_parameter: active_param,
                        }],
                        active_signature: Some(0),
                        active_parameter: active_param,
                    });
                }
            }
        }
    }

    None
}

fn format_signature_label(name: &str, args: &[String]) -> String {
    if args.is_empty() {
        name.to_string()
    } else {
        format!("{}    {}", name, args.join("    "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_signature_help_local_keyword() {
        let src = "*** Keywords ***\nMy Keyword\n    [Arguments]    ${arg1}    ${arg2}\n    Log    ${arg1}\n*** Test Cases ***\nMy Test\n    My Keyword    ";
        let file = parse(src);
        let ns = Namespace::new(None);
        // Position in the args of "My Keyword" call.
        let pos = Position { line: 6, character: 20 };
        let result = signature_help(&file, &ns, pos);
        assert!(result.is_some());
        let sh = result.unwrap();
        assert_eq!(sh.signatures.len(), 1);
        assert!(sh.signatures[0].label.contains("My Keyword"));
    }

    #[test]
    fn test_no_signature_on_keyword_name() {
        let src = "*** Test Cases ***\nMy Test\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        // Position at the start of "Log" keyword.
        let pos = Position { line: 2, character: 4 };
        let result = signature_help(&file, &ns, pos);
        // Cursor is on the keyword name itself, not in args.
        assert!(result.is_none());
    }

    #[test]
    fn test_format_signature_label() {
        assert_eq!(
            format_signature_label("My KW", &["${a}".to_string(), "${b}".to_string()]),
            "My KW    ${a}    ${b}"
        );
        assert_eq!(format_signature_label("Bare KW", &[]), "Bare KW");
    }
}
