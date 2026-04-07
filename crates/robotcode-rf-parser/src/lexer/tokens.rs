//! Token types for the Robot Framework lexer.

/// Source position — all fields are 0-indexed.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// The kind of a [`Token`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // ── Section headers ──────────────────────────────────────────────────
    SettingHeader,
    VariableHeader,
    TestCaseHeader,
    TaskHeader,
    KeywordHeader,
    CommentHeader,

    // ── Settings-section keywords ─────────────────────────────────────────
    Library,
    Resource,
    Variables,
    Documentation,
    Metadata,
    SuiteSetup,
    SuiteTeardown,
    TestSetup,
    TestTeardown,
    TestTemplate,
    TestTags,
    DefaultTags,
    ForceTags,
    KeywordTags,
    TaskTags,

    // ── Variable-section ─────────────────────────────────────────────────
    Variable,

    // ── Block names ──────────────────────────────────────────────────────
    TestCaseName,
    TaskName,
    KeywordName,

    // ── Inline settings ───────────────────────────────────────────────────
    /// `[Arguments]`
    ArgumentsSetting,
    /// `[Documentation]`
    DocumentationSetting,
    /// `[Tags]`
    TagsSetting,
    /// `[Setup]`
    SetupSetting,
    /// `[Teardown]`
    TeardownSetting,
    /// `[Template]`
    TemplateSetting,
    /// `[Timeout]`
    TimeoutSetting,
    /// `[Return]`
    ReturnSetting,
    /// `[Keyword Tags]`
    KeywordTagsSetting,

    // ── Body tokens ───────────────────────────────────────────────────────
    /// A keyword call or argument value.
    Keyword,
    Argument,
    /// An assignment target, e.g. `${var}=`.
    Assign,
    /// `WITH NAME` / `AS` alias in Library imports.
    WithName,

    // ── Control flow ─────────────────────────────────────────────────────
    For,
    ForSeparator,
    While,
    If,
    ElseIf,
    Else,
    Try,
    Except,
    Finally,
    End,
    Break,
    Continue,
    /// The `RETURN` statement (RF6+), distinct from `[Return]` setting.
    ReturnStatement,

    // ── Structural ────────────────────────────────────────────────────────
    Separator,
    Continuation,
    Comment,
    EmptyLine,
    Eol,
    Eos,
    Error,
    Data,
}

/// A single lexer token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub position: Position,
}

impl Token {
    pub fn new(kind: TokenKind, value: impl Into<String>, position: Position) -> Self {
        Self {
            kind,
            value: value.into(),
            position,
        }
    }
}
