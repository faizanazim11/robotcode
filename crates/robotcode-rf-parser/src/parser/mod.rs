//! Recursive-descent Robot Framework parser.

pub mod ast;

use ast::*;

use crate::lexer::{
    tokens::{Position, Token, TokenKind},
};

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse Robot Framework source text into a [`File`] AST.
pub fn parse(source: &str) -> File {
    parse_with_source(source, None)
}

/// Parse with an optional source file name.
pub fn parse_with_source(source: &str, source_name: Option<&str>) -> File {
    let statements = crate::lexer::tokenize(source);
    let mut parser = Parser::new(statements);
    parser.parse_file(source_name)
}

// ── Internal parser ───────────────────────────────────────────────────────────

struct Parser {
    stmts: Vec<Vec<Token>>,
    pos: usize,
}

impl Parser {
    fn new(stmts: Vec<Vec<Token>>) -> Self {
        Self { stmts, pos: 0 }
    }

    // ── Peek / consume ────────────────────────────────────────────────────

    fn peek(&self) -> Option<&Vec<Token>> {
        self.stmts.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Vec<Token>> {
        let stmt = self.stmts.get(self.pos);
        self.pos += 1;
        stmt
    }

    fn at_end(&self) -> bool {
        self.pos >= self.stmts.len()
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    /// First meaningful token kind of the current statement.
    fn first_kind(&self) -> Option<&TokenKind> {
        self.peek().and_then(|s| s.first().map(|t| &t.kind))
    }

    /// True if current statement is a section header.
    fn is_section_header(&self) -> bool {
        matches!(
            self.first_kind(),
            Some(
                TokenKind::SettingHeader
                    | TokenKind::VariableHeader
                    | TokenKind::TestCaseHeader
                    | TokenKind::TaskHeader
                    | TokenKind::KeywordHeader
                    | TokenKind::CommentHeader
            )
        )
    }

    // ── File ──────────────────────────────────────────────────────────────

    fn parse_file(&mut self, source_name: Option<&str>) -> File {
        let mut sections: Vec<Section> = Vec::new();
        let mut pre_section_errors: Vec<ErrorNode> = Vec::new();

        while !self.at_end() {
            if self.is_section_header() {
                break;
            }
            // Content before first section header.
            if let Some(stmt) = self.advance() {
                if !is_empty_or_comment_stmt(stmt) {
                    pre_section_errors.push(ErrorNode {
                        value: stmt_text(stmt),
                        message: "Content before first section".into(),
                        position: stmt_position(stmt),
                    });
                }
            }
        }

        if !pre_section_errors.is_empty() {
            sections.push(Section::Invalid(InvalidSection {
                header: None,
                body: pre_section_errors,
            }));
        }

        while !self.at_end() {
            sections.push(self.parse_section());
        }

        File {
            sections,
            source: source_name.map(|s| s.to_string()),
        }
    }

    // ── Section dispatch ──────────────────────────────────────────────────

    fn parse_section(&mut self) -> Section {
        let header_stmt = self.advance().expect("section header");
        let header_tok = &header_stmt[0];
        let header = SectionHeader {
            name: header_tok.value.clone(),
            position: header_tok.position.clone(),
        };

        match &header_tok.kind {
            TokenKind::SettingHeader => Section::Settings(self.parse_settings(header)),
            TokenKind::VariableHeader => Section::Variables(self.parse_variables(header)),
            TokenKind::TestCaseHeader => Section::TestCases(self.parse_test_cases(header)),
            TokenKind::TaskHeader => Section::Tasks(self.parse_tasks(header)),
            TokenKind::KeywordHeader => Section::Keywords(self.parse_keywords(header)),
            TokenKind::CommentHeader => Section::Comments(self.parse_comments(header)),
            _ => Section::Invalid(InvalidSection {
                header: Some(header),
                body: vec![],
            }),
        }
    }

    // ── Settings section ──────────────────────────────────────────────────

    fn parse_settings(&mut self, header: SectionHeader) -> SettingsSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            let stmt = self.advance().unwrap().clone();
            body.push(self.setting_item_from_stmt(&stmt));
        }
        SettingsSection { header, body }
    }

    fn setting_item_from_stmt(&self, stmt: &[Token]) -> SettingItem {
        if stmt.is_empty() {
            return SettingItem::EmptyLine(EmptyLine { position: Position::default() });
        }
        let first = &stmt[0];
        match &first.kind {
            TokenKind::EmptyLine => {
                SettingItem::EmptyLine(EmptyLine { position: first.position.clone() })
            }
            TokenKind::Comment => SettingItem::Comment(CommentLine {
                value: first.value.clone(),
                position: first.position.clone(),
            }),
            TokenKind::Library => {
                let args_tokens = data_tokens(stmt, 1);
                let (args, alias) = split_with_name(&args_tokens);
                SettingItem::LibraryImport(LibraryImport {
                    name: args_tokens.first().cloned().unwrap_or_default(),
                    args,
                    alias,
                    position: first.position.clone(),
                })
            }
            TokenKind::Resource => SettingItem::ResourceImport(ResourceImport {
                path: data_tokens(stmt, 1).into_iter().next().unwrap_or_default(),
                position: first.position.clone(),
            }),
            TokenKind::Variables => {
                let args = data_tokens(stmt, 1);
                let path = args.first().cloned().unwrap_or_default();
                let rest = args.into_iter().skip(1).collect();
                SettingItem::VariablesImport(VariablesImport {
                    path,
                    args: rest,
                    position: first.position.clone(),
                })
            }
            TokenKind::Documentation => SettingItem::Documentation(Documentation {
                value: data_tokens(stmt, 1).join(" "),
                position: first.position.clone(),
            }),
            TokenKind::Metadata => {
                let args = data_tokens(stmt, 1);
                let name = args.first().cloned().unwrap_or_default();
                let value = args.into_iter().skip(1).collect::<Vec<_>>().join(" ");
                SettingItem::Metadata(Metadata { name, value, position: first.position.clone() })
            }
            TokenKind::SuiteSetup => SettingItem::SuiteSetup(self.fixture(stmt, FixtureKind::SuiteSetup)),
            TokenKind::SuiteTeardown => {
                SettingItem::SuiteTeardown(self.fixture(stmt, FixtureKind::SuiteTeardown))
            }
            TokenKind::TestSetup => SettingItem::TestSetup(self.fixture(stmt, FixtureKind::TestSetup)),
            TokenKind::TestTeardown => {
                SettingItem::TestTeardown(self.fixture(stmt, FixtureKind::TestTeardown))
            }
            TokenKind::TestTemplate => SettingItem::TestTemplate(TestTemplate {
                name: data_tokens(stmt, 1).join(" "),
                position: first.position.clone(),
            }),
            TokenKind::TestTags => SettingItem::TestTags(Tags {
                kind: TagsKind::Test,
                tags: data_tokens(stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::DefaultTags => SettingItem::DefaultTags(Tags {
                kind: TagsKind::Default,
                tags: data_tokens(stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::ForceTags => SettingItem::ForceTags(Tags {
                kind: TagsKind::Force,
                tags: data_tokens(stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::KeywordTags => SettingItem::KeywordTags(Tags {
                kind: TagsKind::Keyword,
                tags: data_tokens(stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::TaskTags => SettingItem::TaskTags(Tags {
                kind: TagsKind::Task,
                tags: data_tokens(stmt, 1),
                position: first.position.clone(),
            }),
            _ => SettingItem::Error(ErrorNode {
                value: stmt_text(stmt),
                message: format!("Unexpected token {:?}", first.kind),
                position: first.position.clone(),
            }),
        }
    }

    fn fixture(&self, stmt: &[Token], kind: FixtureKind) -> SuiteFixture {
        let first = &stmt[0];
        let args = data_tokens(stmt, 1);
        let name = args.first().cloned().unwrap_or_default();
        let rest = args.into_iter().skip(1).collect();
        SuiteFixture { kind, name, args: rest, position: first.position.clone() }
    }

    // ── Variables section ─────────────────────────────────────────────────

    fn parse_variables(&mut self, header: SectionHeader) -> VariablesSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            let stmt = self.advance().unwrap().clone();
            let item = match stmt.first().map(|t| &t.kind) {
                Some(TokenKind::EmptyLine) => {
                    VariableItem::EmptyLine(EmptyLine { position: stmt[0].position.clone() })
                }
                Some(TokenKind::Comment) => VariableItem::Comment(CommentLine {
                    value: stmt[0].value.clone(),
                    position: stmt[0].position.clone(),
                }),
                Some(TokenKind::Variable) => VariableItem::Variable(VariableDecl {
                    name: stmt[0].value.clone(),
                    value: data_tokens(&stmt, 1),
                    position: stmt[0].position.clone(),
                }),
                _ => {
                    if stmt.first().map(|t| &t.kind) == Some(&TokenKind::Eol)
                        || stmt.first().map(|t| &t.kind) == Some(&TokenKind::Eos)
                    {
                        continue;
                    }
                    VariableItem::Error(ErrorNode {
                        value: stmt_text(&stmt),
                        message: "Unexpected token in Variables section".into(),
                        position: stmt_position(&stmt),
                    })
                }
            };
            body.push(item);
        }
        VariablesSection { header, body }
    }

    // ── Test cases section ────────────────────────────────────────────────

    fn parse_test_cases(&mut self, header: SectionHeader) -> TestCasesSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            if matches!(self.first_kind(), Some(TokenKind::EmptyLine)) {
                self.advance();
                continue;
            }
            if matches!(self.first_kind(), Some(TokenKind::TestCaseName)) {
                let name_stmt = self.advance().unwrap().clone();
                let name = name_stmt[0].value.clone();
                let pos = name_stmt[0].position.clone();
                let body_items = self.parse_body(&[TokenKind::TestCaseName]);
                body.push(TestCase { name, position: pos, body: body_items });
            } else {
                self.advance(); // skip unexpected
            }
        }
        TestCasesSection { header, body }
    }

    // ── Tasks section ─────────────────────────────────────────────────────

    fn parse_tasks(&mut self, header: SectionHeader) -> TasksSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            if matches!(self.first_kind(), Some(TokenKind::EmptyLine)) {
                self.advance();
                continue;
            }
            if matches!(self.first_kind(), Some(TokenKind::TaskName)) {
                let name_stmt = self.advance().unwrap().clone();
                let name = name_stmt[0].value.clone();
                let pos = name_stmt[0].position.clone();
                let body_items = self.parse_body(&[TokenKind::TaskName]);
                body.push(Task { name, position: pos, body: body_items });
            } else {
                self.advance();
            }
        }
        TasksSection { header, body }
    }

    // ── Keywords section ──────────────────────────────────────────────────

    fn parse_keywords(&mut self, header: SectionHeader) -> KeywordsSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            if matches!(self.first_kind(), Some(TokenKind::EmptyLine)) {
                self.advance();
                continue;
            }
            if matches!(self.first_kind(), Some(TokenKind::KeywordName)) {
                let name_stmt = self.advance().unwrap().clone();
                let name = name_stmt[0].value.clone();
                let pos = name_stmt[0].position.clone();
                let body_items = self.parse_body(&[TokenKind::KeywordName]);
                body.push(Keyword { name, position: pos, body: body_items });
            } else {
                self.advance();
            }
        }
        KeywordsSection { header, body }
    }

    // ── Comments section ──────────────────────────────────────────────────

    fn parse_comments(&mut self, header: SectionHeader) -> CommentsSection {
        let mut body = Vec::new();
        while !self.at_end() && !self.is_section_header() {
            let stmt = self.advance().unwrap().clone();
            if let Some(t) = stmt.first() {
                body.push(CommentLine { value: t.value.clone(), position: t.position.clone() });
            }
        }
        CommentsSection { header, body }
    }

    // ── Body items (test/keyword) ─────────────────────────────────────────

    /// Parse body items until we hit a section header or a new block name.
    fn parse_body(&mut self, stop_at: &[TokenKind]) -> Vec<BodyItem> {
        let mut items = Vec::new();
        loop {
            if self.at_end() || self.is_section_header() {
                break;
            }
            if let Some(k) = self.first_kind() {
                if stop_at.contains(k) {
                    break;
                }
            }
            let stmt = self.advance().unwrap().clone();
            items.push(self.body_item_from_stmt(stmt, stop_at));
        }
        items
    }

    fn body_item_from_stmt(&mut self, stmt: Vec<Token>, stop_at: &[TokenKind]) -> BodyItem {
        let first = match stmt.first() {
            Some(t) => t,
            None => return BodyItem::EmptyLine(EmptyLine { position: Position::default() }),
        };

        match &first.kind {
            TokenKind::EmptyLine => BodyItem::EmptyLine(EmptyLine { position: first.position.clone() }),
            TokenKind::Comment => BodyItem::Comment(CommentLine {
                value: first.value.clone(),
                position: first.position.clone(),
            }),

            // ── Inline settings ───────────────────────────────────────────
            TokenKind::DocumentationSetting => BodyItem::Documentation(Documentation {
                value: data_tokens(&stmt, 1).join(" "),
                position: first.position.clone(),
            }),
            TokenKind::ArgumentsSetting => BodyItem::Arguments(ArgumentsDef {
                args: data_tokens(&stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::TagsSetting | TokenKind::KeywordTagsSetting => BodyItem::Tags(Tags {
                kind: TagsKind::Inline,
                tags: data_tokens(&stmt, 1),
                position: first.position.clone(),
            }),
            TokenKind::SetupSetting => {
                let args = data_tokens(&stmt, 1);
                let name = args.first().cloned().unwrap_or_default();
                let rest = args.into_iter().skip(1).collect();
                BodyItem::Setup(InlineFixture {
                    kind: InlineFixtureKind::Setup,
                    name,
                    args: rest,
                    position: first.position.clone(),
                })
            }
            TokenKind::TeardownSetting => {
                let args = data_tokens(&stmt, 1);
                let name = args.first().cloned().unwrap_or_default();
                let rest = args.into_iter().skip(1).collect();
                BodyItem::Teardown(InlineFixture {
                    kind: InlineFixtureKind::Teardown,
                    name,
                    args: rest,
                    position: first.position.clone(),
                })
            }
            TokenKind::TemplateSetting => BodyItem::Template(TestTemplate {
                name: data_tokens(&stmt, 1).join(" "),
                position: first.position.clone(),
            }),
            TokenKind::TimeoutSetting => BodyItem::Timeout(Timeout {
                value: data_tokens(&stmt, 1).into_iter().next().unwrap_or_default(),
                position: first.position.clone(),
            }),
            TokenKind::ReturnSetting => BodyItem::ReturnSetting(ReturnSetting {
                values: data_tokens(&stmt, 1),
                position: first.position.clone(),
            }),

            // ── Control flow ──────────────────────────────────────────────
            TokenKind::For => BodyItem::For(self.parse_for(&stmt)),
            TokenKind::While => BodyItem::While(self.parse_while(&stmt, stop_at)),
            TokenKind::If => BodyItem::If(self.parse_if(&stmt, stop_at)),
            TokenKind::Try => BodyItem::Try(self.parse_try(&stmt, stop_at)),
            TokenKind::Break => {
                BodyItem::Break(BreakStmt { position: first.position.clone() })
            }
            TokenKind::Continue => {
                BodyItem::Continue(ContinueStmt { position: first.position.clone() })
            }
            TokenKind::ReturnStatement => BodyItem::Return(ReturnStmt {
                values: data_tokens(&stmt, 1),
                position: first.position.clone(),
            }),

            // ── Keyword call (with optional assignments) ──────────────────
            TokenKind::Assign | TokenKind::Keyword => {
                let assigns: Vec<String> = stmt
                    .iter()
                    .take_while(|t| t.kind == TokenKind::Assign)
                    .map(|t| t.value.clone())
                    .collect();
                let remaining: Vec<&Token> = stmt.iter().skip(assigns.len()).collect();
                let name = remaining
                    .iter()
                    .find(|t| t.kind == TokenKind::Keyword)
                    .map(|t| t.value.clone())
                    .unwrap_or_default();
                let args: Vec<String> = remaining
                    .iter()
                    .skip_while(|t| t.kind != TokenKind::Argument)
                    .filter(|t| t.kind == TokenKind::Argument)
                    .map(|t| t.value.clone())
                    .collect();
                // If first stmt is a keyword call with no assigns, it may be a
                // template argument row (when inside a templated test). We keep
                // it as a KeywordCall — the analyzer decides.
                BodyItem::KeywordCall(KeywordCall {
                    assigns,
                    name,
                    args,
                    position: first.position.clone(),
                })
            }

            _ => BodyItem::Error(ErrorNode {
                value: stmt_text(&stmt),
                message: format!("Unexpected token {:?} in body", first.kind),
                position: first.position.clone(),
            }),
        }
    }

    // ── FOR loop ──────────────────────────────────────────────────────────

    fn parse_for(&mut self, header: &[Token]) -> ForLoop {
        let pos = header[0].position.clone();
        let args = data_tokens(header, 1);

        // Split on IN / IN RANGE / IN ENUMERATE / IN ZIP
        let (vars, flavor, values) = split_for_args(&args);

        let body = self.parse_block_body();
        ForLoop {
            variables: vars,
            flavor,
            values,
            options: vec![],
            body,
            position: pos,
        }
    }

    // ── WHILE loop ────────────────────────────────────────────────────────

    fn parse_while(&mut self, header: &[Token], _stop_at: &[TokenKind]) -> WhileLoop {
        let pos = header[0].position.clone();
        let args = data_tokens(header, 1);
        let condition = args.first().cloned();
        let options = args
            .iter()
            .skip(1)
            .filter_map(|a| {
                let mut parts = a.splitn(2, '=');
                let name = parts.next()?.to_string();
                let value = parts.next()?.to_string();
                Some(ForOption { name, value })
            })
            .collect();
        let body = self.parse_block_body();
        WhileLoop { condition, options, body, position: pos }
    }

    // ── IF block ──────────────────────────────────────────────────────────

    fn parse_if(&mut self, header: &[Token], stop_at: &[TokenKind]) -> IfBlock {
        let pos = header[0].position.clone();
        let condition = data_tokens(header, 1).into_iter().next();
        let mut branches = Vec::new();
        let branch_pos = header[0].position.clone();

        // Collect IF body until ELSE IF / ELSE / END.
        let body = self.parse_until_else_or_end(stop_at);
        branches.push(IfBranch { kind: IfKind::If, condition, body, position: branch_pos });

        // ELSE IF / ELSE branches.
        loop {
            match self.first_kind() {
                Some(TokenKind::ElseIf) => {
                    let s = self.advance().unwrap().clone();
                    let cond = data_tokens(&s, 1).into_iter().next();
                    let bpos = s[0].position.clone();
                    let body = self.parse_until_else_or_end(stop_at);
                    branches.push(IfBranch { kind: IfKind::ElseIf, condition: cond, body, position: bpos });
                }
                Some(TokenKind::Else) => {
                    let s = self.advance().unwrap().clone();
                    let bpos = s[0].position.clone();
                    let body = self.parse_block_body();
                    branches.push(IfBranch { kind: IfKind::Else, condition: None, body, position: bpos });
                    break;
                }
                Some(TokenKind::End) => {
                    self.advance();
                    break;
                }
                _ => break,
            }
        }

        IfBlock { branches, position: pos }
    }

    fn parse_until_else_or_end(&mut self, stop_at: &[TokenKind]) -> Vec<BodyItem> {
        let mut items = Vec::new();
        loop {
            if self.at_end() || self.is_section_header() {
                break;
            }
            match self.first_kind() {
                Some(TokenKind::ElseIf | TokenKind::Else | TokenKind::End) => break,
                Some(k) if stop_at.contains(k) => break,
                _ => {
                    let stmt = self.advance().unwrap().clone();
                    items.push(self.body_item_from_stmt(stmt, stop_at));
                }
            }
        }
        items
    }

    // ── TRY block ─────────────────────────────────────────────────────────

    fn parse_try(&mut self, header: &[Token], stop_at: &[TokenKind]) -> TryBlock {
        let pos = header[0].position.clone();
        let mut branches = Vec::new();
        let bpos = header[0].position.clone();

        let body = self.parse_until_except_or_end(stop_at);
        branches.push(TryBranch {
            kind: TryKind::Try,
            patterns: vec![],
            pattern_type: None,
            var: None,
            body,
            position: bpos,
        });

        loop {
            match self.first_kind() {
                Some(TokenKind::Except) => {
                    let s = self.advance().unwrap().clone();
                    let bpos2 = s[0].position.clone();
                    let args = data_tokens(&s, 1);
                    let (patterns, pattern_type, var) = parse_except_args(args);
                    let body = self.parse_until_except_or_end(stop_at);
                    branches.push(TryBranch { kind: TryKind::Except, patterns, pattern_type, var, body, position: bpos2 });
                }
                Some(TokenKind::Else) => {
                    let s = self.advance().unwrap().clone();
                    let bpos2 = s[0].position.clone();
                    let body = self.parse_until_except_or_end(stop_at);
                    branches.push(TryBranch { kind: TryKind::Else, patterns: vec![], pattern_type: None, var: None, body, position: bpos2 });
                }
                Some(TokenKind::Finally) => {
                    let s = self.advance().unwrap().clone();
                    let bpos2 = s[0].position.clone();
                    let body = self.parse_block_body();
                    branches.push(TryBranch { kind: TryKind::Finally, patterns: vec![], pattern_type: None, var: None, body, position: bpos2 });
                    break;
                }
                Some(TokenKind::End) => {
                    self.advance();
                    break;
                }
                _ => break,
            }
        }

        TryBlock { branches, position: pos }
    }

    fn parse_until_except_or_end(&mut self, stop_at: &[TokenKind]) -> Vec<BodyItem> {
        let mut items = Vec::new();
        loop {
            if self.at_end() || self.is_section_header() {
                break;
            }
            match self.first_kind() {
                Some(TokenKind::Except | TokenKind::Else | TokenKind::Finally | TokenKind::End) => break,
                Some(k) if stop_at.contains(k) => break,
                _ => {
                    let stmt = self.advance().unwrap().clone();
                    items.push(self.body_item_from_stmt(stmt, stop_at));
                }
            }
        }
        items
    }

    /// Consume body tokens until `END`, returning them.
    fn parse_block_body(&mut self) -> Vec<BodyItem> {
        let mut items = Vec::new();
        loop {
            if self.at_end() || self.is_section_header() {
                break;
            }
            if matches!(self.first_kind(), Some(TokenKind::End)) {
                self.advance();
                break;
            }
            let stmt = self.advance().unwrap().clone();
            items.push(self.body_item_from_stmt(stmt, &[]));
        }
        items
    }
}

// ── Static helper functions ───────────────────────────────────────────────────

/// Collect data (non-structural) token values from a statement starting at `from`.
fn data_tokens(stmt: &[Token], from: usize) -> Vec<String> {
    stmt.iter()
        .skip(from)
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Eol | TokenKind::Eos | TokenKind::Separator
            )
        })
        .map(|t| t.value.clone())
        .filter(|v| !v.is_empty())
        .collect()
}

/// Split library args at `WITH NAME` / `AS` returning `(args_before, alias)`.
fn split_with_name(args: &[String]) -> (Vec<String>, Option<String>) {
    for (i, a) in args.iter().enumerate() {
        if a.eq_ignore_ascii_case("WITH NAME") || a.eq_ignore_ascii_case("AS") {
            let before = args[..i].to_vec();
            let alias = args.get(i + 1).cloned();
            return (before, alias);
        }
    }
    (args.to_vec(), None)
}

/// Split FOR loop args into `(variables, flavor, values)`.
fn split_for_args(args: &[String]) -> (Vec<String>, String, Vec<String>) {
    let separator_idx = args.iter().position(|a| {
        let upper = a.to_ascii_uppercase();
        matches!(upper.as_str(), "IN" | "IN RANGE" | "IN ENUMERATE" | "IN ZIP")
    });

    if let Some(idx) = separator_idx {
        let vars = args[..idx].to_vec();
        let flavor = args[idx].clone();
        let values = args[idx + 1..].to_vec();
        (vars, flavor, values)
    } else {
        (args.to_vec(), "IN".to_string(), vec![])
    }
}

/// Parse EXCEPT args: `(patterns, type=..., AS ${var})`.
fn parse_except_args(args: Vec<String>) -> (Vec<String>, Option<String>, Option<String>) {
    let mut patterns = Vec::new();
    let mut pattern_type = None;
    let mut var = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.eq_ignore_ascii_case("AS") {
            var = args.get(i + 1).cloned();
            i += 2;
        } else if a.to_ascii_lowercase().starts_with("type=") {
            pattern_type = Some(a[5..].to_string());
            i += 1;
        } else {
            patterns.push(a.clone());
            i += 1;
        }
    }
    (patterns, pattern_type, var)
}

fn stmt_text(stmt: &[Token]) -> String {
    stmt.iter()
        .filter(|t| !matches!(t.kind, TokenKind::Eol | TokenKind::Eos))
        .map(|t| t.value.as_str())
        .collect::<Vec<_>>()
        .join("    ")
}

fn stmt_position(stmt: &[Token]) -> Position {
    stmt.first().map(|t| t.position.clone()).unwrap_or_default()
}

fn is_empty_or_comment_stmt(stmt: &[Token]) -> bool {
    stmt.iter().all(|t| {
        matches!(t.kind, TokenKind::EmptyLine | TokenKind::Comment | TokenKind::Eol | TokenKind::Eos)
    })
}
