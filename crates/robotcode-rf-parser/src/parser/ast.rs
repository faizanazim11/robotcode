//! AST node types for the Robot Framework parser.

pub use crate::lexer::tokens::Position;

// ── Top-level ─────────────────────────────────────────────────────────────────

/// A parsed `.robot` or `.resource` file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct File {
    pub sections: Vec<Section>,
    pub source: Option<String>,
}

/// One of the top-level sections in an RF file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Section {
    Settings(SettingsSection),
    Variables(VariablesSection),
    TestCases(TestCasesSection),
    Tasks(TasksSection),
    Keywords(KeywordsSection),
    Comments(CommentsSection),
    Invalid(InvalidSection),
}

// ── Section headers ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SectionHeader {
    pub name: String,
    pub position: Position,
}

// ── Settings section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingsSection {
    pub header: SectionHeader,
    pub body: Vec<SettingItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SettingItem {
    LibraryImport(LibraryImport),
    ResourceImport(ResourceImport),
    VariablesImport(VariablesImport),
    Documentation(Documentation),
    Metadata(Metadata),
    SuiteSetup(SuiteFixture),
    SuiteTeardown(SuiteFixture),
    TestSetup(SuiteFixture),
    TestTeardown(SuiteFixture),
    TestTemplate(TestTemplate),
    TestTags(Tags),
    DefaultTags(Tags),
    ForceTags(Tags),
    KeywordTags(Tags),
    TaskTags(Tags),
    Comment(CommentLine),
    EmptyLine(EmptyLine),
    Error(ErrorNode),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryImport {
    pub name: String,
    pub args: Vec<String>,
    pub alias: Option<String>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceImport {
    pub path: String,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariablesImport {
    pub path: String,
    pub args: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Documentation {
    pub value: String,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub name: String,
    pub value: String,
    pub position: Position,
}

/// Suite/test setup or teardown — stores the keyword call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuiteFixture {
    pub kind: FixtureKind,
    pub name: String,
    pub args: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FixtureKind {
    SuiteSetup,
    SuiteTeardown,
    TestSetup,
    TestTeardown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestTemplate {
    pub name: String,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tags {
    pub kind: TagsKind,
    pub tags: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TagsKind {
    Test,
    Default,
    Force,
    Keyword,
    Task,
    Inline,
}

// ── Variables section ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariablesSection {
    pub header: SectionHeader,
    pub body: Vec<VariableItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum VariableItem {
    Variable(VariableDecl),
    Comment(CommentLine),
    EmptyLine(EmptyLine),
    Error(ErrorNode),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariableDecl {
    /// The variable name token value, e.g. `"${NAME}"`.
    pub name: String,
    pub value: Vec<String>,
    pub position: Position,
}

// ── Test cases section ────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestCasesSection {
    pub header: SectionHeader,
    pub body: Vec<TestCase>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestCase {
    pub name: String,
    pub position: Position,
    pub body: Vec<BodyItem>,
}

// ── Tasks section ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TasksSection {
    pub header: SectionHeader,
    pub body: Vec<Task>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub name: String,
    pub position: Position,
    pub body: Vec<BodyItem>,
}

// ── Keywords section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeywordsSection {
    pub header: SectionHeader,
    pub body: Vec<Keyword>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Keyword {
    pub name: String,
    pub position: Position,
    pub body: Vec<BodyItem>,
}

// ── Comments section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentsSection {
    pub header: SectionHeader,
    pub body: Vec<CommentLine>,
}

// ── Invalid / pre-section content ─────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvalidSection {
    pub header: Option<SectionHeader>,
    pub body: Vec<ErrorNode>,
}

// ── Body items (shared by TestCase, Task, Keyword) ────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum BodyItem {
    // Inline settings
    Documentation(Documentation),
    Arguments(ArgumentsDef),
    Tags(Tags),
    Setup(InlineFixture),
    Teardown(InlineFixture),
    Template(TestTemplate),
    Timeout(Timeout),
    ReturnSetting(ReturnSetting),
    // Statements
    KeywordCall(KeywordCall),
    TemplateArguments(TemplateArguments),
    // Control flow
    For(ForLoop),
    While(WhileLoop),
    If(IfBlock),
    Try(TryBlock),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Return(ReturnStmt),
    // Trivia
    Comment(CommentLine),
    EmptyLine(EmptyLine),
    Error(ErrorNode),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArgumentsDef {
    pub args: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InlineFixture {
    pub kind: InlineFixtureKind,
    pub name: String,
    pub args: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InlineFixtureKind {
    Setup,
    Teardown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timeout {
    pub value: String,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReturnSetting {
    pub values: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeywordCall {
    pub assigns: Vec<String>,
    pub name: String,
    pub args: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateArguments {
    pub args: Vec<String>,
    pub position: Position,
}

// ── Control flow ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForLoop {
    pub variables: Vec<String>,
    pub flavor: String,
    pub values: Vec<String>,
    pub options: Vec<ForOption>,
    pub body: Vec<BodyItem>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForOption {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WhileLoop {
    pub condition: Option<String>,
    pub options: Vec<ForOption>,
    pub body: Vec<BodyItem>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IfBlock {
    pub branches: Vec<IfBranch>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IfBranch {
    pub kind: IfKind,
    pub condition: Option<String>,
    pub body: Vec<BodyItem>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IfKind {
    If,
    ElseIf,
    Else,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TryBlock {
    pub branches: Vec<TryBranch>,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TryBranch {
    pub kind: TryKind,
    pub patterns: Vec<String>,
    pub pattern_type: Option<String>,
    pub var: Option<String>,
    pub body: Vec<BodyItem>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TryKind {
    Try,
    Except,
    Else,
    Finally,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BreakStmt {
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContinueStmt {
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReturnStmt {
    pub values: Vec<String>,
    pub position: Position,
}

// ── Shared trivial nodes ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentLine {
    pub value: String,
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmptyLine {
    pub position: Position,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorNode {
    pub value: String,
    pub message: String,
    pub position: Position,
}
