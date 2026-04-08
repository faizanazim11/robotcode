//! `textDocument/semanticTokens/full` handler.
//!
//! Produces a **delta-encoded** semantic token stream for the full document.
//!
//! ## Token type legend (indices match `LEGEND_TOKEN_TYPES` order)
//! | Index | `SemanticTokenType`  | Used for                                          |
//! |-------|----------------------|---------------------------------------------------|
//! | 0     | `namespace`          | Section headers (`*** Settings ***`, …)           |
//! | 1     | `function`           | Keyword calls and keyword definition names        |
//! | 2     | `variable`           | Variable declarations and `${var}` references     |
//! | 3     | `keyword`            | Setting names (`Library`, `Resource`, …)          |
//! | 4     | `comment`            | Comment lines                                     |
//! | 5     | `string`             | Documentation values                              |
//!
//! ## Token modifier legend (bit positions)
//! | Bit | `SemanticTokenModifier` | Used for                              |
//! |-----|-------------------------|---------------------------------------|
//! | 0   | `definition`            | Variable/keyword definition sites     |
//! | 1   | `deprecated`            | Deprecated keyword calls              |

use lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend,
};
use robotcode_rf_parser::parser::ast::{
    BodyItem, File, Section, SettingItem, VariableItem,
};
use robotcode_rf_parser::variables::search_variable;

// ── Legend ────────────────────────────────────────────────────────────────────

/// The ordered list of semantic token types advertised in `ServerCapabilities`.
pub const LEGEND_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE, // 0 – section header
    SemanticTokenType::FUNCTION,  // 1 – keyword call / definition
    SemanticTokenType::VARIABLE,  // 2 – variable ref / declaration
    SemanticTokenType::KEYWORD,   // 3 – setting name
    SemanticTokenType::COMMENT,   // 4 – comment
    SemanticTokenType::STRING,    // 5 – documentation / string value
];

/// The ordered list of semantic token modifiers advertised in `ServerCapabilities`.
pub const LEGEND_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DEFINITION, // 0
    SemanticTokenModifier::DEPRECATED, // 1
];

/// Build the `SemanticTokensLegend` to include in `ServerCapabilities`.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: LEGEND_TOKEN_TYPES.to_vec(),
        token_modifiers: LEGEND_TOKEN_MODIFIERS.to_vec(),
    }
}

// Token type constants for readability.
const TT_NAMESPACE: u32 = 0;
const TT_FUNCTION: u32 = 1;
const TT_VARIABLE: u32 = 2;
const TT_KEYWORD: u32 = 3;
const TT_COMMENT: u32 = 4;
const TT_STRING: u32 = 5;

// Token modifier bit masks.
const TM_DEFINITION: u32 = 1 << 0;

// ── Raw token (before delta-encoding) ────────────────────────────────────────

#[derive(Debug, Clone)]
struct RawToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Compute semantic tokens for the entire `file`.
pub fn semantic_tokens(file: &File) -> SemanticTokens {
    let mut raw: Vec<RawToken> = Vec::new();
    collect_file(&mut raw, file);

    // Sort by position (they should already be in order, but be defensive).
    raw.sort_by_key(|t| (t.line, t.start_char));

    // Delta-encode.
    let mut data: Vec<SemanticToken> = Vec::with_capacity(raw.len());
    let mut prev_line: u32 = 0;
    let mut prev_char: u32 = 0;

    for tok in &raw {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start_char - prev_char
        } else {
            tok.start_char
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.token_modifiers,
        });
        prev_line = tok.line;
        prev_char = tok.start_char;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

// ── Collectors ────────────────────────────────────────────────────────────────

fn collect_file(out: &mut Vec<RawToken>, file: &File) {
    for section in &file.sections {
        match section {
            Section::Settings(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for item in &s.body {
                    collect_setting(out, item);
                }
            }
            Section::Variables(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for item in &s.body {
                    if let VariableItem::Variable(v) = item {
                        push(out, v.position.line, v.position.column, v.name.len() as u32, TT_VARIABLE, TM_DEFINITION);
                    } else if let VariableItem::Comment(c) = item {
                        push_comment(out, &c.value, c.position.line, c.position.column);
                    }
                }
            }
            Section::TestCases(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for tc in &s.body {
                    push(out, tc.position.line, tc.position.column, tc.name.len() as u32, TT_FUNCTION, TM_DEFINITION);
                    collect_body(out, &tc.body);
                }
            }
            Section::Tasks(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for task in &s.body {
                    push(out, task.position.line, task.position.column, task.name.len() as u32, TT_FUNCTION, TM_DEFINITION);
                    collect_body(out, &task.body);
                }
            }
            Section::Keywords(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for kw in &s.body {
                    push(out, kw.position.line, kw.position.column, kw.name.len() as u32, TT_FUNCTION, TM_DEFINITION);
                    collect_body(out, &kw.body);
                }
            }
            Section::Comments(s) => {
                push(out, s.header.position.line, s.header.position.column, s.header.name.len() as u32, TT_NAMESPACE, 0);
                for c in &s.body {
                    push_comment(out, &c.value, c.position.line, c.position.column);
                }
            }
            Section::Invalid(_) => {}
        }
    }
}

fn collect_setting(out: &mut Vec<RawToken>, item: &SettingItem) {
    match item {
        SettingItem::LibraryImport(x) => {
            push(out, x.position.line, x.position.column, "Library".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Library".len() as u32 + 4, x.name.len() as u32, TT_STRING, 0);
        }
        SettingItem::ResourceImport(x) => {
            push(out, x.position.line, x.position.column, "Resource".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Resource".len() as u32 + 4, x.path.len() as u32, TT_STRING, 0);
        }
        SettingItem::VariablesImport(x) => {
            push(out, x.position.line, x.position.column, "Variables".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Variables".len() as u32 + 4, x.path.len() as u32, TT_STRING, 0);
        }
        SettingItem::Documentation(x) => {
            push(out, x.position.line, x.position.column, "Documentation".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Documentation".len() as u32 + 4, x.value.len() as u32, TT_STRING, 0);
        }
        SettingItem::SuiteSetup(x) => {
            push(out, x.position.line, x.position.column, "Suite Setup".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Suite Setup".len() as u32 + 4, x.name.len() as u32, TT_FUNCTION, 0);
        }
        SettingItem::SuiteTeardown(x) => {
            push(out, x.position.line, x.position.column, "Suite Teardown".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Suite Teardown".len() as u32 + 4, x.name.len() as u32, TT_FUNCTION, 0);
        }
        SettingItem::TestSetup(x) => {
            push(out, x.position.line, x.position.column, "Test Setup".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Test Setup".len() as u32 + 4, x.name.len() as u32, TT_FUNCTION, 0);
        }
        SettingItem::TestTeardown(x) => {
            push(out, x.position.line, x.position.column, "Test Teardown".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Test Teardown".len() as u32 + 4, x.name.len() as u32, TT_FUNCTION, 0);
        }
        SettingItem::TestTemplate(x) => {
            push(out, x.position.line, x.position.column, "Test Template".len() as u32, TT_KEYWORD, 0);
            push(out, x.position.line, x.position.column + "Test Template".len() as u32 + 4, x.name.len() as u32, TT_FUNCTION, 0);
        }
        SettingItem::Metadata(x) => {
            push(out, x.position.line, x.position.column, "Metadata".len() as u32, TT_KEYWORD, 0);
        }
        SettingItem::TestTags(x) | SettingItem::DefaultTags(x) | SettingItem::ForceTags(x)
        | SettingItem::KeywordTags(x) | SettingItem::TaskTags(x) => {
            push(out, x.position.line, x.position.column, x.kind_name().len() as u32, TT_KEYWORD, 0);
        }
        SettingItem::Comment(c) => {
            push_comment(out, &c.value, c.position.line, c.position.column);
        }
        SettingItem::EmptyLine(_) | SettingItem::Error(_) => {}
    }
}

fn collect_body(out: &mut Vec<RawToken>, items: &[BodyItem]) {
    for item in items {
        collect_body_item(out, item);
    }
}

fn collect_body_item(out: &mut Vec<RawToken>, item: &BodyItem) {
    match item {
        BodyItem::KeywordCall(kc) => {
            // Assignments: `${var} =`
            for assign in &kc.assigns {
                push(out, kc.position.line, kc.position.column, assign.len() as u32, TT_VARIABLE, 0);
            }
            let name_col = if kc.assigns.is_empty() {
                kc.position.column
            } else {
                // Heuristic: put the keyword name after the last assignment.
                // Exact column is hard without the raw text, so approximate.
                kc.position.column + kc.assigns.iter().map(|a| a.len() as u32 + 4).sum::<u32>()
            };
            push(out, kc.position.line, name_col, kc.name.len() as u32, TT_FUNCTION, 0);
            // Highlight variable references in arguments.
            for arg in &kc.args {
                collect_variable_refs(out, arg, kc.position.line);
            }
        }
        BodyItem::Arguments(a) => {
            for arg in &a.args {
                push(out, a.position.line, a.position.column, arg.len() as u32, TT_VARIABLE, TM_DEFINITION);
            }
        }
        BodyItem::Documentation(d) => {
            push(out, d.position.line, d.position.column, "Documentation".len() as u32, TT_KEYWORD, 0);
            push(out, d.position.line, d.position.column + "Documentation".len() as u32 + 4, d.value.len() as u32, TT_STRING, 0);
        }
        BodyItem::Tags(t) => {
            push(out, t.position.line, t.position.column, t.kind_name().len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::Setup(f) | BodyItem::Teardown(f) => {
            let label = f.kind_name();
            push(out, f.position.line, f.position.column, label.len() as u32, TT_KEYWORD, 0);
            push(out, f.position.line, f.position.column + label.len() as u32 + 4, f.name.len() as u32, TT_FUNCTION, 0);
        }
        BodyItem::Template(t) => {
            push(out, t.position.line, t.position.column, "Template".len() as u32, TT_KEYWORD, 0);
            push(out, t.position.line, t.position.column + "Template".len() as u32 + 4, t.name.len() as u32, TT_FUNCTION, 0);
        }
        BodyItem::Comment(c) => {
            push_comment(out, &c.value, c.position.line, c.position.column);
        }
        BodyItem::For(f) => {
            push(out, f.position.line, f.position.column, "FOR".len() as u32, TT_KEYWORD, 0);
            for var in &f.variables {
                push(out, f.position.line, f.position.column, var.len() as u32, TT_VARIABLE, 0);
            }
            collect_body(out, &f.body);
        }
        BodyItem::While(w) => {
            push(out, w.position.line, w.position.column, "WHILE".len() as u32, TT_KEYWORD, 0);
            collect_body(out, &w.body);
        }
        BodyItem::If(iblk) => {
            for branch in &iblk.branches {
                let kw = match branch.kind {
                    robotcode_rf_parser::parser::ast::IfKind::If => "IF",
                    robotcode_rf_parser::parser::ast::IfKind::ElseIf => "ELSE IF",
                    robotcode_rf_parser::parser::ast::IfKind::Else => "ELSE",
                };
                push(out, branch.position.line, branch.position.column, kw.len() as u32, TT_KEYWORD, 0);
                collect_body(out, &branch.body);
            }
        }
        BodyItem::Try(tblk) => {
            for branch in &tblk.branches {
                let kw = match branch.kind {
                    robotcode_rf_parser::parser::ast::TryKind::Try => "TRY",
                    robotcode_rf_parser::parser::ast::TryKind::Except => "EXCEPT",
                    robotcode_rf_parser::parser::ast::TryKind::Else => "ELSE",
                    robotcode_rf_parser::parser::ast::TryKind::Finally => "FINALLY",
                };
                push(out, branch.position.line, branch.position.column, kw.len() as u32, TT_KEYWORD, 0);
                collect_body(out, &branch.body);
            }
        }
        BodyItem::Return(r) => {
            push(out, r.position.line, r.position.column, "RETURN".len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::Break(b) => {
            push(out, b.position.line, b.position.column, "BREAK".len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::Continue(c) => {
            push(out, c.position.line, c.position.column, "CONTINUE".len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::ReturnSetting(r) => {
            push(out, r.position.line, r.position.column, "Return".len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::Timeout(t) => {
            push(out, t.position.line, t.position.column, "Timeout".len() as u32, TT_KEYWORD, 0);
        }
        BodyItem::TemplateArguments(_) | BodyItem::EmptyLine(_) | BodyItem::Error(_) => {}
    }
}

/// Collect `${var}` and `@{var}` references inside `text`.
fn collect_variable_refs(out: &mut Vec<RawToken>, text: &str, line: u32) {
    let mut remaining = text;
    let mut offset: u32 = 0;
    while let Some(m) = search_variable(remaining) {
        let start = offset + m.start as u32;
        let len = (m.end - m.start) as u32;
        push(out, line, start, len, TT_VARIABLE, 0);
        let consumed = m.end;
        if consumed >= remaining.len() {
            break;
        }
        offset += consumed as u32;
        remaining = &remaining[consumed..];
    }
}

fn push(out: &mut Vec<RawToken>, line: u32, start_char: u32, length: u32, token_type: u32, token_modifiers: u32) {
    if length == 0 {
        return;
    }
    out.push(RawToken { line, start_char, length, token_type, token_modifiers });
}

fn push_comment(out: &mut Vec<RawToken>, value: &str, line: u32, col: u32) {
    push(out, line, col, value.len() as u32, TT_COMMENT, 0);
}

// ── Helper trait extensions ───────────────────────────────────────────────────

trait TagsKindName {
    fn kind_name(&self) -> &'static str;
}

impl TagsKindName for robotcode_rf_parser::parser::ast::Tags {
    fn kind_name(&self) -> &'static str {
        use robotcode_rf_parser::parser::ast::TagsKind;
        match self.kind {
            TagsKind::Test => "Test Tags",
            TagsKind::Default => "Default Tags",
            TagsKind::Force => "Force Tags",
            TagsKind::Keyword => "Keyword Tags",
            TagsKind::Task => "Task Tags",
            TagsKind::Inline => "Tags",
        }
    }
}

trait InlineFixtureKindName {
    fn kind_name(&self) -> &'static str;
}

impl InlineFixtureKindName for robotcode_rf_parser::parser::ast::InlineFixture {
    fn kind_name(&self) -> &'static str {
        use robotcode_rf_parser::parser::ast::InlineFixtureKind;
        match self.kind {
            InlineFixtureKind::Setup => "[Setup]",
            InlineFixtureKind::Teardown => "[Teardown]",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_rf_parser::parser::parse;

    #[test]
    fn test_semantic_tokens_non_empty() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hello\n";
        let file = parse(src);
        let tokens = semantic_tokens(&file);
        assert!(!tokens.data.is_empty());
    }

    #[test]
    fn test_semantic_tokens_ordering() {
        let src = "*** Settings ***\nLibrary    Collections\n\n*** Keywords ***\nMy Keyword\n    Log    hello\n";
        let file = parse(src);
        let tokens = semantic_tokens(&file);
        // Delta-encoded: reconstruct absolute positions and verify they're sorted.
        let mut line: u32 = 0;
        let mut char: u32 = 0;
        let mut positions: Vec<(u32, u32)> = Vec::new();
        for tok in &tokens.data {
            line += tok.delta_line;
            if tok.delta_line > 0 {
                char = tok.delta_start;
            } else {
                char += tok.delta_start;
            }
            positions.push((line, char));
        }
        let sorted: Vec<(u32, u32)> = {
            let mut s = positions.clone();
            s.sort();
            s
        };
        assert_eq!(positions, sorted, "Semantic tokens must be sorted by position");
    }

    #[test]
    fn test_legend_types_count() {
        assert_eq!(LEGEND_TOKEN_TYPES.len(), 6);
        assert_eq!(LEGEND_TOKEN_MODIFIERS.len(), 2);
    }
}
