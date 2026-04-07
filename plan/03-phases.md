# 03 — Phased Migration Roadmap

## Overview

The migration is organized into **8 phases**. Each phase produces a shippable artifact that can be tested independently. The Python packages remain functional throughout — the Rust implementation is an additive layer, not a hard cutover until Phase 8.

**Total estimated effort**: 18–24 months (2–3 full-time engineers)

---

## Phase 1 — Foundation & Cargo Workspace
**Duration**: 4–6 weeks  
**Goal**: Establish the Rust project infrastructure and core data types.

### Deliverables

- [x] Initialize `Cargo.toml` workspace at repo root
- [x] Create `crates/robotcode-core/` with:
  - [x] `uri.rs` — URI parsing and normalization (port of `core/uri.py`)
  - [x] `text_document.rs` — UTF-16 text document with incremental edits using `ropey`
  - [x] `lsp_types.rs` — Re-export `lsp-types` crate; add any custom extensions
  - [x] `workspace.rs` — Multi-root workspace model
  - [x] `documents_manager.rs` — Thread-safe open document registry (`DashMap`)
  - [x] `async_tools.rs` — Cancellation tokens, async mutex helpers
  - [x] `event.rs` — Event/callback system
  - [x] `filewatcher.rs` — File system watching using `notify`
  - [x] `utils/logging.rs` — `tracing` subscriber setup
  - [x] `utils/path.rs` — File ID utilities (inode-stable file identity)
  - [x] `utils/dataclasses.rs` — Common serde helpers
- [x] Set up CI: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check` (`.github/workflows/rust-checks.yml`)
- [x] Add `Cargo.toml` to `.gitignore` exclusions appropriately (`target/` already excluded)
- [x] Establish snapshot test infrastructure using `insta` crate

### Success Criteria
- `cargo build` succeeds from clean checkout ✅
- All `robotcode-core` unit tests pass (38 tests: 29 unit + 3 doc + 6 snapshot) ✅
- CI pipeline runs Rust checks alongside existing Python checks ✅

---

## Phase 2 — Robot Framework Parser (Rust-Native)
**Duration**: 8–10 weeks  
**Goal**: Implement a complete, error-recovering `.robot`/`.resource` file parser in Rust. This is the highest-leverage change — the Python parser is the primary performance bottleneck for large workspaces.

### Deliverables

- [x] Create `crates/robotcode-rf-parser/`
- [x] **Lexer** (`lexer/`):
  - [x] Token type enum mirroring `robot.parsing.lexer.tokens.Token`
  - [x] Context-sensitive line-based scanner for RF tokenization (indent-sensitive, header keywords, etc.)
  - [x] Section modes: Settings, Variables, TestCases, Tasks, Keywords, Comments
  - [x] Error token handling (non-crashing on malformed input)
- [x] **Parser** (`parser/`):
  - [x] Complete AST node hierarchy (see below)
  - [x] Recursive-descent parser producing typed AST from token stream
  - [x] Error recovery: skip to next logical boundary on syntax error
  - [x] Preserve all source position information (line, column, end_line, end_column)
  - [x] Trivia preservation (comments, whitespace) for formatter use
- [x] **AST Node Types** (mirror `robot.parsing.model.*`):
  - [x] `File`, `SettingSection`, `VariableSection`, `TestCaseSection`, `KeywordSection`, `CommentSection`
  - [x] Settings: `LibraryImport`, `ResourceImport`, `VariablesImport`, `Documentation`, `Tags`, `Suite Setup/Teardown`, `Test Setup/Teardown`, `Test Template`, `Force/Default Tags`, `Metadata`
  - [x] `Variable` (variable declaration)
  - [x] `TestCase`, `Keyword` (block nodes)
  - [x] Statements: `KeywordCall`, `Arguments`, `ReturnStatement`, `IfHeader`/`ElseIfHeader`/`ElseHeader`/`EndHeader`, `ForHeader`/`WhileHeader`/`TryHeader`/`ExceptHeader`/`FinallyHeader`, `BreakStatement`, `ContinueStatement`, `TemplateArguments`, `Comment`, `EmptyLine`
- [x] **Variable Utilities** (`variables.rs`):
  - [x] Port `robot.variables.search` — `is_variable`, `search_variable`, `contains_variable`, `is_scalar_assign`
  - [x] Variable types: scalar `${x}`, list `@{x}`, dict `&{x}`, env `%{x}`
- [x] **Escaping** (`escaping.rs`): Port `robot.utils.escaping.unescape` and `split_from_equals`
- [x] **Multi-version support** (`versions.rs`): RF 5.x / 6.x / 7.x syntax differences
- [x] **Visitor trait** (`visitor.rs`): Generic `AstVisitor` trait with default no-op implementations
- [x] **Snapshot tests**: 5 snapshot tests covering simple, variables, settings, keywords, and control-flow fixtures

### AST Compatibility Note
The Rust AST does not need to be a 1:1 mirror of Robot Framework's Python AST — it only needs to expose the same **semantic information**. Internal structure can be Rust-idiomatic (e.g., enums instead of class hierarchies).

### Success Criteria
- All `.robot`/`.resource` test files in `tests/` parse without panic
- Snapshot AST output matches Python parser output for all test files
- Parser throughput ≥ 10× faster than Python `robot.api.parsing.get_model()` on benchmark corpus
- Zero-allocation hot path for the common case (cached documents)

---

## Phase 3 — JSON-RPC 2.0 & LSP Transport
**Duration**: 3–4 weeks  
**Goal**: Implement the async JSON-RPC 2.0 server and wire up `tower-lsp`.

### Deliverables

- [ ] Create `crates/robotcode-jsonrpc2/` (thin wrapper — most work is tower-lsp):
  - [ ] Stdio and TCP transports (tokio)
  - [ ] `@rpc_method` equivalent: Rust proc-macro attribute or manual dispatch table
- [ ] Create `crates/robotcode-language-server/` skeleton:
  - [ ] `tower-lsp` `LanguageServer` trait implementation
  - [ ] `initialize` / `initialized` / `shutdown` / `exit` handlers
  - [ ] `textDocument/didOpen` / `didChange` / `didClose` / `didSave` handlers
  - [ ] Document-change event pipeline → triggers re-analysis
- [ ] Create binary crate `crates/robotcode/`:
  - [ ] `clap` CLI with `language-server` subcommand
  - [ ] `--stdio` / `--tcp PORT` transport flags
  - [ ] `--python PATH` flag (Python interpreter for bridge)
- [ ] **Smoke test**: Connect VS Code to the Rust language server stub; verify `initialize` handshake succeeds and documents sync (no actual diagnostics yet)

### Success Criteria
- VS Code can connect to the Rust binary as a language server
- Open/close/change events are received and logged
- No crashes on any valid LSP message sequence

---

## Phase 4 — Python Bridge & Library Introspection
**Duration**: 4–5 weeks  
**Goal**: Implement the Python bridge for Robot Framework library introspection.

### Deliverables

- [ ] Create `python-bridge/helper.py`:
  - [ ] JSON-over-stdio request/response loop
  - [ ] `library_doc` method: wraps `robot.libdocpkg.LibraryDocumentation`
  - [ ] `variables_doc` method: loads RF variables files
  - [ ] `embedded_args` method: wraps `robot.running.arguments.embedded.EmbeddedArguments`
  - [ ] `normalize` method: wraps `robot.utils.NormalizedDict`/`normalize`
  - [ ] `rf_version` method: returns installed RF version
  - [ ] `discover` method: wraps `robot.running.builder.TestSuiteBuilder`
  - [ ] Error handling: returns JSON error for any Python exception
- [ ] Create `crates/robotcode-python-bridge/`:
  - [ ] `Bridge` trait: `async fn call(&self, method, params) -> Result<Value>`
  - [ ] `SubprocessBridge`: spawns `python helper.py`, communicates via JSON stdio
  - [ ] `MockBridge`: for unit testing without Python
  - [ ] Connection lifecycle: start-on-demand, restart-on-crash, idle timeout
  - [ ] Per-workspace bridge instances (each workspace may have a different venv)
- [ ] Create `crates/robotcode-robot/diagnostics/library_doc.rs`:
  - [ ] `LibraryDoc` struct (mirrors Python `LibraryDoc` dataclass)
  - [ ] `KeywordDoc` struct with argument spec
  - [ ] `ArgumentSpec`, `ArgInfo` structs
  - [ ] `EmbeddedArgument` struct + regex matching
  - [ ] Bridge call to fetch `LibraryDoc` from Python; cache by (library_name, args, python_path)
- [ ] **Integration test**: Load `BuiltIn`, `Collections`, `String`, `OperatingSystem` standard RF libraries via bridge; verify keyword count and argument signatures match Python reference output

### Success Criteria
- All RF standard library keyword docs load correctly via bridge
- Bridge restarts gracefully after Python crash
- Cache hit rate >95% for typical workspace
- Library load latency ≤ 50ms (Python startup amortized over workspace session)

---

## Phase 5 — Diagnostics Engine
**Duration**: 8–10 weeks  
**Goal**: Implement the core analysis engine: namespace analysis, import resolution, variable scope — the heart of the language server.

### Deliverables

- [ ] `crates/robotcode-robot/diagnostics/entities.rs`:
  - [ ] `LibraryEntry`, `ResourceEntry`, `VariablesEntry`
  - [ ] `LibraryImport`, `ResourceImport`, `VariablesImport`
  - [ ] `KeywordDoc` with full `ArgumentSpec`
- [ ] `crates/robotcode-robot/diagnostics/errors.rs`:
  - [ ] All diagnostic codes and message templates (must match Python exactly)
  - [ ] `DiagnosticSeverity` assignments
- [ ] `crates/robotcode-robot/diagnostics/import_resolver.rs`:
  - [ ] Resolve `Library`, `Resource`, `Variables` import paths
  - [ ] Handle `PYTHONPATH`, `sys.path`, robot.toml `python-path` config
  - [ ] Circular import detection
  - [ ] Workspace-relative and absolute path resolution
- [ ] `crates/robotcode-robot/diagnostics/imports_manager.rs`:
  - [ ] Async cache of resolved imports (keyed by (path, args, python_path))
  - [ ] Invalidation on file change events
  - [ ] Parallel import resolution with `tokio::spawn`
- [ ] `crates/robotcode-robot/diagnostics/variable_scope.rs`:
  - [ ] RF variable scoping rules (global, suite, test, local)
  - [ ] Variable assignment tracking in keyword/test bodies
  - [ ] `FOR`, `WHILE`, `TRY` scope handling
  - [ ] `Set Variable`, `Set Suite Variable`, `Set Global Variable` keyword tracking
- [ ] `crates/robotcode-robot/diagnostics/keyword_finder.rs`:
  - [ ] Find keyword definition by name (normalized, embedded args)
  - [ ] Disambiguation across multiple libraries
  - [ ] Embedded argument regex matching
- [ ] `crates/robotcode-robot/diagnostics/namespace.rs`:
  - [ ] `Namespace` struct: merged view of all imports for one file
  - [ ] Keyword lookup, variable lookup, import lookup
- [ ] `crates/robotcode-robot/diagnostics/namespace_analyzer.rs`:
  - [ ] Walk AST, emit `Diagnostic` structs
  - [ ] Undefined keyword detection
  - [ ] Undefined variable detection
  - [ ] Import error reporting
  - [ ] Argument count/type mismatch detection
  - [ ] Duplicate keyword names
  - [ ] RF version-specific warnings
- [ ] `crates/robotcode-robot/diagnostics/document_cache.rs`:
  - [ ] Per-document analysis cache (`Arc<RwLock<DocumentAnalysis>>`)
  - [ ] Invalidation cascade (changing a library invalidates all files importing it)
- [ ] Wire diagnostics into language server: push diagnostics on document open/change/save

### Success Criteria
- Diagnostics output (codes, ranges, severity, messages) exactly matches Python implementation on all test fixtures
- Workspace-wide analysis of 500-file RF project completes in <2 seconds (Python baseline: ~15 seconds)
- Incremental re-analysis after single file change completes in <100ms
- Zero false positives compared to Python reference implementation on test corpus

---

## Phase 6 — LSP Feature Parity
**Duration**: 10–12 weeks  
**Goal**: Implement all LSP language features.

### Deliverables (each as a sub-task)

#### Text Document Features
- [ ] **Semantic tokens** (`semantic_tokens.rs`):
  - [ ] Full token type legend (must match Python exactly)
  - [ ] Keyword names, variable references, settings, section headers, comments
- [ ] **Document symbols** (`document_symbols.rs`):
  - [ ] Test cases, keywords, variables as symbol hierarchy
- [ ] **Folding ranges** (`folding_range.rs`):
  - [ ] Sections, test cases, keywords, block constructs (FOR, IF, TRY, WHILE)
- [ ] **Document highlight** (`highlight.rs`):
  - [ ] Highlight all references to token under cursor
- [ ] **Selection range** (`selection_range.rs`)
- [ ] **Inlay hints** (`inlay_hints.rs`):
  - [ ] Argument names in keyword calls

#### Navigation Features
- [ ] **Go-to-definition** (`goto.rs`):
  - [ ] Keyword definitions (same file, resources, libraries)
  - [ ] Variable definitions
  - [ ] Import file paths (→ open resource file)
- [ ] **Go-to-declaration** / **Go-to-implementation**
- [ ] **Find references** (`references.rs`):
  - [ ] All usages of a keyword or variable across workspace
- [ ] **Workspace symbols** (`workspace_symbols.rs`)
- [ ] **Rename** (`rename.rs`):
  - [ ] Rename keyword, rename variable (workspace-wide)

#### Completion & Hints
- [ ] **Completion** (`completion.rs`):
  - [ ] Keyword completion (with argument snippets)
  - [ ] Variable completion
  - [ ] Setting name completion
  - [ ] Library name completion (scan venv site-packages)
  - [ ] Resource file path completion
  - [ ] BDD-style (`Given`/`When`/`Then`) keyword completion
- [ ] **Hover** (`hover.rs`):
  - [ ] Keyword signature and documentation (Markdown)
  - [ ] Variable value/type hints
  - [ ] Import documentation
- [ ] **Signature help** (`signature_help.rs`):
  - [ ] Active argument highlighting in keyword calls

#### Code Actions & Formatting
- [ ] **Code actions — quick fixes** (`code_actions.rs`):
  - [ ] Add missing library import
  - [ ] Fix keyword name typo (Levenshtein-distance suggestion)
  - [ ] Create missing resource file
- [ ] **Code actions — refactoring** (`code_actions.rs`):
  - [ ] Extract keyword
  - [ ] Inline keyword
- [ ] **Code lens** (`code_lens.rs`):
  - [ ] Run test / Debug test lenses on test cases
- [ ] **Formatting** (`formatting.rs`):
  - [ ] RF file formatting (consistent spacing, alignment)
  - [ ] Respect `.editorconfig`
- [ ] **Documentation HTTP server** (optional): Serve keyword HTML docs in browser

### Success Criteria
- All existing snapshot tests pass against Rust implementation
- VS Code integration test suite passes (manual verification)
- Feature parity verified by running existing Python language server test suite against Rust binary

---

## Phase 7 — Debug Adapter Protocol & CLI Tools
**Duration**: 6–8 weeks  
**Goal**: Implement the DAP server and remaining CLI tools.

### Deliverables

#### DAP Server
- [ ] `crates/robotcode-debugger/dap_types.rs` — DAP 1.51 type model
- [ ] `crates/robotcode-debugger/server.rs` — DAP stdio/TCP server
- [ ] `crates/robotcode-debugger/protocol.rs` — DAP message dispatcher
- [ ] `crates/robotcode-debugger/debugger.rs`:
  - [ ] Launch RF in Python subprocess with debug listener injected
  - [ ] Breakpoint setting (line breakpoints, conditional breakpoints)
  - [ ] Step over / step into / step out
  - [ ] Stack frame inspection
  - [ ] Variable inspection (RF variables, Python local variables)
  - [ ] Exception breakpoints (on RF failures)
  - [ ] Pause / continue / disconnect
  - [ ] Output events (test log → DAP OutputEvent)
- [ ] `crates/robotcode-debugger/launcher/` — Launch configuration (attach, launch modes)

#### CLI Tools
- [ ] `crates/robotcode-runner/`:
  - [ ] `robotcode run` — wraps Python `robot.run` via bridge
  - [ ] `robotcode rebot` — wraps Python `robot.rebot` via bridge
  - [ ] `robotcode libdoc` — wraps Python `robot.libdoc` via bridge
  - [ ] `robotcode testdoc` — wraps Python `robot.testdoc` via bridge
  - [ ] `robotcode discover` — Rust-native test discovery using RF parser
- [ ] `crates/robotcode-analyze/`:
  - [ ] `robotcode analyze` — batch static analysis, exit code for CI
  - [ ] `robotcode analyze cache` — cache management
- [ ] Final `clap` CLI with all subcommands registered

### Success Criteria
- Debug session can set breakpoints, step through RF test execution
- `robotcode discover` output matches Python implementation on all test suites
- `robotcode analyze` exit codes match Python implementation

---

## Phase 8 — REPL, Integration & Cutover
**Duration**: 4–6 weeks  
**Goal**: Implement REPL server, complete VS Code/IntelliJ integration, deprecate Python packages.

### Deliverables

- [ ] `crates/robotcode-repl/`:
  - [ ] REPL server (JSON-RPC over stdio/TCP)
  - [ ] Keyword evaluation via Python bridge (`robot.run` single-keyword mode)
  - [ ] History, completion, result display
- [ ] VS Code extension updates:
  - [ ] `languageclientsmanger.ts`: prefer Rust binary, fall back to Python
  - [ ] `pythonmanger.ts`: pass `--python` to Rust binary
  - [ ] `debugmanager.ts`: use Rust DAP binary
  - [ ] `package.json`: bundle Rust binary for Linux/macOS/Windows
  - [ ] CI: cross-compile Rust binary for `x86_64-linux`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`
- [ ] IntelliJ plugin updates:
  - [ ] Update server launch command
  - [ ] Update Gradle config for binary distribution
- [ ] `bundled/` directory update:
  - [ ] Remove Python language server from bundled libs
  - [ ] Add Rust binary (platform-specific) to bundled resources
  - [ ] Keep `python-bridge/helper.py` in bundled libs
- [ ] Deprecation notices in Python packages
- [ ] Migration guide for users running language server directly
- [ ] Update documentation (README, CONTRIBUTING, docs/)

### Success Criteria
- Full end-to-end test: open RF project in VS Code, all LSP features work via Rust binary
- Performance benchmarks documented (see [05-performance.md](05-performance.md))
- All existing CI tests pass
- Extension publishes to VS Code Marketplace and IntelliJ Marketplace

---

## Phase Summary Table

| Phase | Name | Duration | Key Output | Status |
|-------|------|----------|------------|--------|
| 1 | Foundation | 4–6 weeks | Cargo workspace, core crate | ✅ Complete |
| 2 | RF Parser | 8–10 weeks | Rust `.robot` parser | ✅ Complete |
| 3 | LSP Transport | 3–4 weeks | `tower-lsp` stub connected to VS Code | |
| 4 | Python Bridge | 4–5 weeks | Library introspection working | |
| 5 | Diagnostics Engine | 8–10 weeks | Diagnostics parity with Python | |
| 6 | LSP Features | 10–12 weeks | Full feature parity | |
| 7 | DAP & CLI | 6–8 weeks | Debugger + CLI tools | |
| 8 | REPL & Cutover | 4–6 weeks | Shipped Rust binary, Python deprecated | |
| **Total** | | **~18–24 months** | | |

---

## Milestone Checkpoints

### M1 (after Phase 2): Parser Validation ✅
- Rust parser handles all test fixtures without panic ✅
- 5 snapshot tests cover all major RF constructs (simple, variables, settings, keywords, control flow) ✅
- Settings and Variables sections produce structured AST nodes (no Error nodes) ✅
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all pass ✅

### M2 (after Phase 4): Language Server Alpha
- VS Code can connect to Rust LS
- Basic diagnostics from imported libraries work
- Internal team testing begins

### M3 (after Phase 5): Diagnostics Beta
- All diagnostic codes match Python reference
- Opt-in beta available to community

### M4 (after Phase 6): Feature Complete Beta
- All LSP features working
- Community testing, snapshot tests passing
- Performance benchmarks published

### M5 (after Phase 8): GA Release
- Rust binary ships in extension
- Python packages marked deprecated
- 6-month parallel support window, then Python packages archived
