//! `textDocument/hover` handler.
//!
//! Returns Markdown documentation for:
//! - Keyword calls → keyword signature + documentation
//! - Variable references → variable name and scope info

use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};
use robotcode_rf_parser::parser::ast::{BodyItem, File, Section};
use robotcode_rf_parser::variables::search_variable;
use robotcode_robot::diagnostics::{entities::KeywordDoc, keyword_finder::KeywordMatch, Namespace};

use super::utils::{text_lines, token_cols};

/// Compute hover content for the token at `pos` in `file`.
///
/// `text` is the raw document source used to compute accurate argument column offsets
/// so that variable references inside keyword-call arguments are detected correctly.
pub fn hover(file: &File, text: &str, ns: &Namespace, pos: Position) -> Option<Hover> {
    let lines = text_lines(text);
    let token = token_at_position(file, &lines, pos)?;
    match token {
        HoverToken::Keyword(name) => hover_for_keyword(ns, &name, file),
        HoverToken::Variable(name) => hover_for_variable(&name, file),
    }
}

// ── Token resolution ──────────────────────────────────────────────────────────

#[derive(Debug)]
enum HoverToken {
    Keyword(String),
    Variable(String),
}

fn token_at_position(file: &File, lines: &[&str], pos: Position) -> Option<HoverToken> {
    for section in &file.sections {
        match section {
            Section::TestCases(s) => {
                for tc in &s.body {
                    if let Some(t) = find_in_body(&tc.body, lines, pos) {
                        return Some(t);
                    }
                }
            }
            Section::Tasks(s) => {
                for task in &s.body {
                    if let Some(t) = find_in_body(&task.body, lines, pos) {
                        return Some(t);
                    }
                }
            }
            Section::Keywords(s) => {
                for kw in &s.body {
                    if let Some(t) = find_in_body(&kw.body, lines, pos) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_body(items: &[BodyItem], lines: &[&str], pos: Position) -> Option<HoverToken> {
    for item in items {
        match item {
            BodyItem::KeywordCall(kc) => {
                if kc.position.line == pos.line {
                    let name_col = kc.position.column;
                    let name_end = name_col + kc.name.len() as u32;
                    if pos.character >= name_col && pos.character <= name_end {
                        return Some(HoverToken::Keyword(kc.name.clone()));
                    }
                    // Check variable references inside arguments using accurate columns.
                    let line_text = lines.get(kc.position.line as usize).copied().unwrap_or("");
                    let cols = token_cols(line_text);
                    let name_idx = kc.assigns.len();
                    for (i, arg) in kc.args.iter().enumerate() {
                        let base_col = cols.get(name_idx + 1 + i).copied().unwrap_or(0);
                        if let Some(var_name) = var_at_position(arg, pos, base_col) {
                            return Some(HoverToken::Variable(var_name));
                        }
                    }
                }
            }
            BodyItem::For(f) => {
                if let Some(t) = find_in_body(&f.body, lines, pos) {
                    return Some(t);
                }
            }
            BodyItem::While(w) => {
                if let Some(t) = find_in_body(&w.body, lines, pos) {
                    return Some(t);
                }
            }
            BodyItem::If(iblk) => {
                for branch in &iblk.branches {
                    if let Some(t) = find_in_body(&branch.body, lines, pos) {
                        return Some(t);
                    }
                }
            }
            BodyItem::Try(tblk) => {
                for branch in &tblk.branches {
                    if let Some(t) = find_in_body(&branch.body, lines, pos) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Return the normalized variable name if `pos` falls on a variable reference in `text`.
///
/// `base_col` is the document-line column where `text` starts.
fn var_at_position(text: &str, pos: Position, base_col: u32) -> Option<String> {
    let mut remaining = text;
    let mut offset: u32 = 0;
    while let Some(m) = search_variable(remaining) {
        let start_char = base_col + offset + m.start as u32;
        let end_char = base_col + offset + m.end as u32;
        if pos.character >= start_char && pos.character <= end_char {
            let var_text = &remaining[m.start..m.end];
            return Some(var_text.to_string());
        }
        if m.end >= remaining.len() {
            break;
        }
        offset += m.end as u32;
        remaining = &remaining[m.end..];
    }
    None
}

// ── Hover content builders ────────────────────────────────────────────────────

fn hover_for_keyword(ns: &Namespace, name: &str, file: &File) -> Option<Hover> {
    // Try namespace first.
    if let KeywordMatch::Found(kw) = ns.find_keyword(name) {
        return Some(build_keyword_hover(kw));
    }

    // Fall back to local keywords in the file.
    for section in &file.sections {
        if let Section::Keywords(s) = section {
            for kw in &s.body {
                if kw.name == name {
                    let sig = format_local_keyword_signature(kw);
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: sig,
                        }),
                        range: None,
                    });
                }
            }
        }
    }

    None
}

fn build_keyword_hover(kw: &KeywordDoc) -> Hover {
    let mut md = String::new();

    // Signature line: `**Keyword Name** (*arg1*, *arg2*, ...)`
    md.push_str(&format!("**{}**", kw.name));
    if !kw.args.is_empty() {
        let args: Vec<String> = kw.args.iter().map(|a| format!("*{}*", a.name)).collect();
        md.push_str(&format!(" ({})", args.join(", ")));
    }
    md.push('\n');

    if let Some(lib) = &kw.library_name {
        md.push_str(&format!("\n*Library: {}*\n", lib));
    }

    if !kw.doc.is_empty() {
        md.push_str("\n---\n\n");
        md.push_str(&kw.doc);
    }

    if let Some(deprecated) = &kw.deprecated {
        md.push_str("\n\n> ⚠️ **Deprecated**: ");
        md.push_str(deprecated);
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: None,
    }
}

fn format_local_keyword_signature(kw: &robotcode_rf_parser::parser::ast::Keyword) -> String {
    // Extract [Arguments] from the keyword body.
    let args: Vec<String> = kw
        .body
        .iter()
        .find_map(|item| {
            if let BodyItem::Arguments(a) = item {
                Some(a.args.iter().map(|s| format!("*{}*", s)).collect())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let mut md = format!("**{}**", kw.name);
    if !args.is_empty() {
        md.push_str(&format!(" ({})", args.join(", ")));
    }

    // Extract [Documentation].
    let doc: Option<String> = kw.body.iter().find_map(|item| {
        if let BodyItem::Documentation(d) = item {
            Some(d.value.clone())
        } else {
            None
        }
    });

    if let Some(doc) = doc {
        if !doc.is_empty() {
            md.push_str("\n\n---\n\n");
            md.push_str(&doc);
        }
    }

    md
}

fn hover_for_variable(name: &str, file: &File) -> Option<Hover> {
    // Look in the Variables section.
    for section in &file.sections {
        if let Section::Variables(s) = section {
            for item in &s.body {
                if let robotcode_rf_parser::parser::ast::VariableItem::Variable(v) = item {
                    let norm = normalize_var_name(&v.name);
                    if norm == normalize_var_name(name) {
                        let value_str = if v.value.is_empty() {
                            "(no value)".to_string()
                        } else {
                            v.value.join("    ")
                        };
                        let md = format!("**{}** = `{}`\n\n*Suite variable*", v.name, value_str);
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: md,
                            }),
                            range: None,
                        });
                    }
                }
            }
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

    #[test]
    fn test_hover_local_keyword() {
        let src = "*** Test Cases ***\nMy Test\n    My Keyword\n*** Keywords ***\nMy Keyword\n    [Documentation]    Does something useful\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 2,
            character: 4,
        };
        let result = hover(&file, src, &ns, pos);
        assert!(result.is_some());
        if let Some(h) = result {
            if let HoverContents::Markup(mc) = h.contents {
                assert!(mc.value.contains("My Keyword"));
            }
        }
    }

    #[test]
    fn test_hover_nothing_on_header() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let file = parse(src);
        let ns = Namespace::new(None);
        let pos = Position {
            line: 0,
            character: 0,
        };
        let result = hover(&file, src, &ns, pos);
        assert!(result.is_none());
    }
}
