//! Robot Framework lexer.
//!
//! The lexer converts raw source text into a flat list of *statements*, where
//! each statement is a `Vec<Token>`.  Lines that start with `...` (continuation
//! marker) are folded into the previous statement.
//!
//! Token separation: RF uses **2 or more spaces** or **a single tab** as the
//! column separator inside a line.  A single space is NOT a separator (it is
//! part of the token value).

pub mod tokens;

pub use tokens::{Position, Token, TokenKind};

// ── Public types ─────────────────────────────────────────────────────────────

/// The section kind currently being lexed (drives context-sensitive behaviour).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    None,
    Settings,
    Variables,
    TestCases,
    Tasks,
    Keywords,
    Comments,
}

/// Tokenize `source` and return a list of *statements*.
///
/// Each inner `Vec` represents one logical statement (which may span multiple
/// physical lines due to `...` continuation).  The last token in each statement
/// is `TokenKind::Eol` or `TokenKind::Eos`.
pub fn tokenize(source: &str) -> Vec<Vec<Token>> {
    Lexer::new(source).tokenize()
}

// ── Internal helpers ─────────────────────────────────────────────────────────

struct Lexer<'s> {
    source: &'s str,
    section: SectionKind,
}

impl<'s> Lexer<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            section: SectionKind::None,
        }
    }

    fn tokenize(mut self) -> Vec<Vec<Token>> {
        let mut statements: Vec<Vec<Token>> = Vec::new();
        let mut current_stmt: Vec<Token> = Vec::new();

        // Collect all physical lines with their line numbers first.
        let physical_lines: Vec<(u32, &str)> = self
            .source
            .lines()
            .enumerate()
            .map(|(i, l)| (i as u32, l))
            .collect();

        for (ln, raw_line) in &physical_lines {
            let line_no = *ln;
            let line = *raw_line;

            // ── 1. Empty line ─────────────────────────────────────────────
            if line.trim().is_empty() {
                // Flush any pending statement first.
                if !current_stmt.is_empty() {
                    push_eol(&mut current_stmt, line_no, line.len() as u32);
                    statements.push(std::mem::take(&mut current_stmt));
                }
                // Emit the empty-line as its own single-token statement.
                let tok = Token::new(
                    TokenKind::EmptyLine,
                    line,
                    pos(line_no, 0, line_no, line.len() as u32),
                );
                statements.push(vec![tok, eol_token(line_no, line.len() as u32)]);
                continue;
            }

            // ── 2. Section header: `*** Xyz ***` ─────────────────────────
            if let Some(kind) = detect_section_header(line) {
                // Flush pending statement.
                if !current_stmt.is_empty() {
                    push_eol(&mut current_stmt, line_no, 0);
                    statements.push(std::mem::take(&mut current_stmt));
                }
                self.section = section_kind_from_header(&kind);
                let tok = Token::new(
                    kind,
                    line.trim(),
                    pos(line_no, 0, line_no, line.len() as u32),
                );
                statements.push(vec![tok, eol_token(line_no, line.len() as u32)]);
                continue;
            }

            // ── 3. Split line into raw cells ──────────────────────────────
            let cells = split_line(line);
            if cells.is_empty() {
                continue;
            }

            // ── 4. Continuation line (`...`) ──────────────────────────────
            let first_cell = cells[0].0.trim();
            if first_cell == "..." {
                // Append remaining cells to current statement (skip the `...` cell).
                for (cell, col) in cells.iter().skip(1) {
                    let t = cell_to_token(
                        cell,
                        line_no,
                        *col,
                        &self.section,
                        false, // not start of statement
                        &current_stmt,
                    );
                    if t.kind != TokenKind::EmptyLine {
                        current_stmt.push(t);
                    }
                }
                continue;
            }

            // ── 5. Comment line ───────────────────────────────────────────
            if first_cell.starts_with('#') {
                if !current_stmt.is_empty() {
                    push_eol(&mut current_stmt, line_no, 0);
                    statements.push(std::mem::take(&mut current_stmt));
                }
                let col = cells[0].1;
                let tok = Token::new(
                    TokenKind::Comment,
                    line.trim_start(),
                    pos(line_no, col, line_no, line.len() as u32),
                );
                statements.push(vec![tok, eol_token(line_no, line.len() as u32)]);
                continue;
            }

            // ── 6. Normal statement line ──────────────────────────────────
            // Flush previous statement if any.
            if !current_stmt.is_empty() {
                push_eol(&mut current_stmt, line_no, 0);
                statements.push(std::mem::take(&mut current_stmt));
            }

            // Does this line START with a separator (i.e., indented)?
            let is_indented = line.starts_with("  ") || line.starts_with('\t');

            for (idx, (cell, col)) in cells.iter().enumerate() {
                if cell.trim().is_empty() {
                    continue;
                }
                let _is_first = idx == 0 || (idx == 1 && cells[0].0.trim().is_empty());
                let t = cell_to_token(
                    cell,
                    line_no,
                    *col,
                    &self.section,
                    !is_indented && idx == 0,
                    &current_stmt,
                );
                current_stmt.push(t);
            }
        }

        // Flush final statement.
        if !current_stmt.is_empty() {
            let last_line = physical_lines.last().map(|(l, _)| *l).unwrap_or(0);
            push_eos(&mut current_stmt, last_line);
            statements.push(current_stmt);
        }

        statements
    }
}

// ── Line splitting ────────────────────────────────────────────────────────────

/// Split a line on 2+ spaces or a single tab, returning `(cell, start_col)` pairs.
fn split_line(line: &str) -> Vec<(String, u32)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut start_col: u32 = 0;
    let mut col: u32 = 0;
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if ch == '\t' {
            // Tab is always a separator.
            if !current.is_empty() || result.is_empty() {
                result.push((current.clone(), start_col));
                current.clear();
            }
            col += 1;
            start_col = col;
            i += 1;
        } else if ch == ' ' {
            // Count consecutive spaces.
            let mut space_count = 0;
            let mut j = i;
            while j < len && chars[j] == ' ' {
                space_count += 1;
                j += 1;
            }
            if space_count >= 2 {
                // Separator.
                if !current.is_empty() || result.is_empty() {
                    result.push((current.clone(), start_col));
                    current.clear();
                }
                col += space_count as u32;
                start_col = col;
                i = j;
            } else {
                // Single space — part of the token.
                current.push(ch);
                col += 1;
                i += 1;
            }
        } else {
            current.push(ch);
            col += 1;
            i += 1;
        }
    }

    if !current.is_empty() {
        result.push((current, start_col));
    }

    result
}

// ── Section header detection ─────────────────────────────────────────────────

/// Returns the `TokenKind` for a section header line, or `None`.
fn detect_section_header(line: &str) -> Option<TokenKind> {
    let t = line.trim();
    if !t.starts_with('*') {
        return None;
    }
    // Strip leading and trailing `*` characters and spaces.
    let inner = t
        .trim_matches(|c| c == '*' || c == ' ')
        .to_ascii_lowercase();
    let inner = inner.trim();
    match inner {
        "settings" | "setting" => Some(TokenKind::SettingHeader),
        "variables" | "variable" => Some(TokenKind::VariableHeader),
        "test cases" | "test case" => Some(TokenKind::TestCaseHeader),
        "tasks" | "task" => Some(TokenKind::TaskHeader),
        "keywords" | "keyword" => Some(TokenKind::KeywordHeader),
        "comments" | "comment" => Some(TokenKind::CommentHeader),
        _ => Some(TokenKind::Error), // unknown section header
    }
}

fn section_kind_from_header(kind: &TokenKind) -> SectionKind {
    match kind {
        TokenKind::SettingHeader => SectionKind::Settings,
        TokenKind::VariableHeader => SectionKind::Variables,
        TokenKind::TestCaseHeader => SectionKind::TestCases,
        TokenKind::TaskHeader => SectionKind::Tasks,
        TokenKind::KeywordHeader => SectionKind::Keywords,
        TokenKind::CommentHeader => SectionKind::Comments,
        _ => SectionKind::None,
    }
}

// ── Context-sensitive cell → Token ───────────────────────────────────────────

fn cell_to_token(
    cell: &str,
    line: u32,
    col: u32,
    section: &SectionKind,
    is_block_name: bool,
    stmt_so_far: &[Token],
) -> Token {
    let end_col = col + cell.chars().count() as u32;
    let p = pos(line, col, line, end_col);
    let v = cell.to_string();

    // Settings section — first cell of statement is the setting keyword.
    // This must be checked before the generic is_block_name path so that rows
    // like `Library    Collections` are not mistakenly tokenized as Data.
    if *section == SectionKind::Settings && stmt_so_far.is_empty() {
        let kind = match_setting_keyword(&v);
        return Token::new(kind, v, p);
    }

    // Variable section — first cell is the variable name.
    if *section == SectionKind::Variables && stmt_so_far.is_empty() {
        return Token::new(TokenKind::Variable, v, p);
    }

    // Block name (test/keyword/task name — non-indented first cell in the
    // TestCases, Tasks, or Keywords sections only).
    if is_block_name {
        let kind = match section {
            SectionKind::TestCases => TokenKind::TestCaseName,
            SectionKind::Tasks => TokenKind::TaskName,
            SectionKind::Keywords => TokenKind::KeywordName,
            _ => TokenKind::Data,
        };
        return Token::new(kind, v, p);
    }

    // Inline setting: `[...]`
    if let Some(kind) = match_inline_setting(&v) {
        return Token::new(kind, v, p);
    }

    // Control flow keywords (in test/keyword body).
    if stmt_so_far.is_empty() || is_first_body_token(stmt_so_far) {
        if let Some(kind) = match_control_flow(&v) {
            return Token::new(kind, v, p);
        }
        // Check for assignment: `${var}=` or `${var} =`
        if is_assignment_token(&v) {
            return Token::new(TokenKind::Assign, v, p);
        }
        // First non-assign, non-control token → keyword call.
        if !stmt_so_far.is_empty() && stmt_so_far.iter().all(|t| t.kind == TokenKind::Assign) {
            return Token::new(TokenKind::Keyword, v, p);
        }
        if stmt_so_far.is_empty() {
            return Token::new(TokenKind::Keyword, v, p);
        }
    }

    // Everything else is an argument.
    Token::new(TokenKind::Argument, v, p)
}

/// Returns true if we're at the "first body token" position (after any assigns).
fn is_first_body_token(stmt_so_far: &[Token]) -> bool {
    stmt_so_far.iter().all(|t| t.kind == TokenKind::Assign)
}

fn match_setting_keyword(s: &str) -> TokenKind {
    match s.to_ascii_lowercase().trim_end_matches(':') {
        "library" => TokenKind::Library,
        "resource" => TokenKind::Resource,
        "variables" => TokenKind::Variables,
        "documentation" => TokenKind::Documentation,
        "metadata" => TokenKind::Metadata,
        "suite setup" => TokenKind::SuiteSetup,
        "suite teardown" => TokenKind::SuiteTeardown,
        "test setup" => TokenKind::TestSetup,
        "test teardown" => TokenKind::TestTeardown,
        "test template" => TokenKind::TestTemplate,
        "test tags" => TokenKind::TestTags,
        "default tags" => TokenKind::DefaultTags,
        "force tags" => TokenKind::ForceTags,
        "keyword tags" => TokenKind::KeywordTags,
        "task tags" => TokenKind::TaskTags,
        _ => TokenKind::Error,
    }
}

fn match_inline_setting(s: &str) -> Option<TokenKind> {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "[arguments]" => Some(TokenKind::ArgumentsSetting),
        "[documentation]" => Some(TokenKind::DocumentationSetting),
        "[tags]" => Some(TokenKind::TagsSetting),
        "[setup]" => Some(TokenKind::SetupSetting),
        "[teardown]" => Some(TokenKind::TeardownSetting),
        "[template]" => Some(TokenKind::TemplateSetting),
        "[timeout]" => Some(TokenKind::TimeoutSetting),
        "[return]" => Some(TokenKind::ReturnSetting),
        "[keyword tags]" => Some(TokenKind::KeywordTagsSetting),
        _ => None,
    }
}

fn match_control_flow(s: &str) -> Option<TokenKind> {
    match s.to_ascii_uppercase().as_str() {
        "FOR" => Some(TokenKind::For),
        "WHILE" => Some(TokenKind::While),
        "IF" => Some(TokenKind::If),
        "ELSE IF" => Some(TokenKind::ElseIf),
        "ELSE" => Some(TokenKind::Else),
        "TRY" => Some(TokenKind::Try),
        "EXCEPT" => Some(TokenKind::Except),
        "FINALLY" => Some(TokenKind::Finally),
        "END" => Some(TokenKind::End),
        "BREAK" => Some(TokenKind::Break),
        "CONTINUE" => Some(TokenKind::Continue),
        "RETURN" => Some(TokenKind::ReturnStatement),
        _ => None,
    }
}

fn is_assignment_token(s: &str) -> bool {
    let s = s.trim_end_matches('=').trim_end();
    (s.starts_with("${") || s.starts_with("@{") || s.starts_with("&{")) && s.ends_with('}')
}

// ── Position helpers ──────────────────────────────────────────────────────────

fn pos(line: u32, col: u32, end_line: u32, end_col: u32) -> Position {
    Position {
        line,
        column: col,
        end_line,
        end_column: end_col,
    }
}

fn eol_token(line: u32, col: u32) -> Token {
    Token::new(TokenKind::Eol, "", pos(line, col, line, col))
}

fn push_eol(stmt: &mut Vec<Token>, line: u32, col: u32) {
    stmt.push(eol_token(line, col));
}

fn push_eos(stmt: &mut Vec<Token>, line: u32) {
    stmt.push(Token::new(TokenKind::Eos, "", pos(line, 0, line, 0)));
}
