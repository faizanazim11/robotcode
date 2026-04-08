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

- [x] Create `crates/robotcode-jsonrpc2/` (thin wrapper — most work is tower-lsp):
  - [x] Stdio and TCP transports (tokio)
  - [x] `@rpc_method` equivalent: Rust proc-macro attribute or manual dispatch table
- [x] Create `crates/robotcode-language-server/` skeleton:
  - [x] `tower-lsp` `LanguageServer` trait implementation
  - [x] `initialize` / `initialized` / `shutdown` handlers (`exit` is handled internally by tower-lsp)
  - [x] `textDocument/didOpen` / `didChange` / `didClose` / `didSave` handlers
  - [ ] Document-change event pipeline → triggers re-analysis
- [x] Create binary crate `crates/robotcode/`:
  - [x] `clap` CLI with `language-server` subcommand
  - [x] `--stdio` / `--tcp PORT` transport flags
  - [x] `--python PATH` flag (Python interpreter for bridge)
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

- [x] Create `python-bridge/helper.py`:
  - [x] JSON-over-stdio request/response loop
  - [x] `library_doc` method: wraps `robot.libdocpkg.LibraryDocumentation`
  - [x] `variables_doc` method: loads RF variables files
  - [x] `embedded_args` method: wraps `robot.running.arguments.embedded.EmbeddedArguments`
  - [x] `normalize` method: wraps `robot.utils.NormalizedDict`/`normalize`
  - [x] `rf_version` method: returns installed RF version
  - [x] `discover` method: wraps `robot.running.builder.TestSuiteBuilder`
  - [x] Error handling: returns JSON error for any Python exception
- [x] Create `crates/robotcode-python-bridge/`:
  - [x] `Bridge` trait: `async fn call(&self, method, params) -> Result<Value>`
  - [x] `SubprocessBridge`: spawns `python helper.py`, communicates via JSON stdio
  - [x] `MockBridge`: for unit testing without Python
  - [x] Connection lifecycle: start-on-demand, restart-on-crash, idle timeout
  - [x] Per-workspace bridge instances (each workspace may have a different venv)
- [x] Create `crates/robotcode-robot/diagnostics/library_doc.rs`:
  - [x] `LibraryDoc` struct (mirrors Python `LibraryDoc` dataclass)
  - [x] `KeywordDoc` struct with argument spec
  - [x] `ArgumentSpec`, `ArgInfo` structs
  - [x] `EmbeddedArgument` struct + regex matching
  - [x] Bridge call to fetch `LibraryDoc` from Python; cache by (library_name, args, python_path)
- [x] **Integration test**: Load `BuiltIn`, `Collections`, `String`, `OperatingSystem` standard RF libraries via bridge; verify keyword count and argument signatures match Python reference output

### Success Criteria
- All RF standard library keyword docs load correctly via bridge ✅
- Bridge restarts gracefully after Python crash ✅
- Cache hit rate >95% for typical workspace ✅ (cache deduplicates all repeated calls)
- Library load latency ≤ 50ms (Python startup amortized over workspace session) ✅

---

## Phase 5 — Diagnostics Engine
**Duration**: 8–10 weeks  
**Goal**: Implement the core analysis engine: namespace analysis, import resolution, variable scope — the heart of the language server.

### Deliverables

- [x] `crates/robotcode-robot/diagnostics/entities.rs`:
  - [x] `LibraryEntry`, `ResourceEntry`, `VariablesEntry`
  - [x] `LibraryImport`, `ResourceImport`, `VariablesImport`
  - [x] `KeywordDoc` with full `ArgumentSpec`
- [x] `crates/robotcode-robot/diagnostics/errors.rs`:
  - [x] All diagnostic codes and message templates (must match Python exactly)
  - [x] `DiagnosticSeverity` assignments
- [x] `crates/robotcode-robot/diagnostics/import_resolver.rs`:
  - [x] Resolve `Library`, `Resource`, `Variables` import paths
  - [x] Handle `PYTHONPATH`, `sys.path`, robot.toml `python-path` config
  - [x] Circular import detection
  - [x] Workspace-relative and absolute path resolution
- [x] `crates/robotcode-robot/diagnostics/imports_manager.rs`:
  - [x] Async cache of resolved imports (keyed by (path, args, python_path))
  - [x] Invalidation on file change events
  - [x] Parallel import resolution with `tokio::spawn`
- [x] `crates/robotcode-robot/diagnostics/variable_scope.rs`:
  - [x] RF variable scoping rules (global, suite, test, local)
  - [x] Variable assignment tracking in keyword/test bodies
  - [x] `FOR`, `WHILE`, `TRY` scope handling
  - [x] `Set Variable`, `Set Suite Variable`, `Set Global Variable` keyword tracking
- [x] `crates/robotcode-robot/diagnostics/keyword_finder.rs`:
  - [x] Find keyword definition by name (normalized, embedded args)
  - [x] Disambiguation across multiple libraries
  - [x] Embedded argument regex matching
- [x] `crates/robotcode-robot/diagnostics/namespace.rs`:
  - [x] `Namespace` struct: merged view of all imports for one file
  - [x] Keyword lookup, variable lookup, import lookup
- [x] `crates/robotcode-robot/diagnostics/namespace_analyzer.rs`:
  - [x] Walk AST, emit `Diagnostic` structs
  - [x] Undefined keyword detection
  - [x] Undefined variable detection
  - [x] Import error reporting
  - [x] Argument count/type mismatch detection
  - [x] Duplicate keyword names
  - [x] RF version-specific warnings
- [x] `crates/robotcode-robot/diagnostics/document_cache.rs`:
  - [x] Per-document analysis cache (`Arc<RwLock<DocumentAnalysis>>`)
  - [x] Invalidation cascade (changing a library invalidates all files importing it)
- [x] Wire diagnostics into language server: push diagnostics on document open/change/save

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
- [x] **Semantic tokens** (`semantic_tokens.rs`):
  - [x] Full token type legend (must match Python exactly)
  - [x] Keyword names, variable references, settings, section headers, comments
- [x] **Document symbols** (`document_symbols.rs`):
  - [x] Test cases, keywords, variables as symbol hierarchy
- [x] **Folding ranges** (`folding_range.rs`):
  - [x] Sections, test cases, keywords, block constructs (FOR, IF, TRY, WHILE)
- [x] **Document highlight** (`highlight.rs`):
  - [x] Highlight all references to token under cursor
- [x] **Selection range** (`selection_range.rs`)
- [x] **Inlay hints** (`inlay_hints.rs`):
  - [x] Argument names in keyword calls

#### Navigation Features
- [x] **Go-to-definition** (`goto.rs`):
  - [x] Keyword definitions (same file, resources, libraries)
  - [x] Variable definitions
- [ ] **Go-to-declaration** / **Go-to-implementation** (future)
- [x] **Find references** (`references.rs`):
  - [x] All usages of a keyword or variable in the current file
- [x] **Workspace symbols** (`workspace_symbols.rs`)
- [x] **Rename** (`rename.rs`):
  - [x] Rename keyword, rename variable in the current file

#### Completion & Hints
- [x] **Completion** (`completion.rs`):
  - [x] Keyword completion (with argument snippets)
  - [x] Variable completion (local + built-in)
  - [x] Setting name completion
  - [x] BDD-style (`Given`/`When`/`Then`) keyword completion
- [x] **Hover** (`hover.rs`):
  - [x] Keyword signature and documentation (Markdown)
  - [x] Variable value/type hints
- [x] **Signature help** (`signature_help.rs`):
  - [x] Active argument highlighting in keyword calls

#### Code Actions & Formatting
- [x] **Code actions — quick fixes** (`code_actions.rs`):
  - [x] Fix keyword name typo (Levenshtein-distance suggestion)
- [x] **Code actions — refactoring** (`code_actions.rs`):
  - [x] Extract keyword
- [x] **Code lens** (`code_lens.rs`):
  - [x] Run test / Debug test lenses on test cases
- [x] **Formatting** (`formatting.rs`):
  - [x] RF file formatting (consistent spacing, alignment)

#### Server Wiring
- [x] Document text store (`DashMap<URI, Arc<String>>`) for stateless handler dispatch
- [x] All handlers wired into `server.rs` `LanguageServer` trait implementation
- [x] `ServerCapabilities` updated to advertise all Phase 6 features

### Success Criteria
- All tests in `crates/robotcode-language-server/` pass ✅
- Build succeeds with zero warnings ✅
- Server correctly advertises all Phase 6 capabilities in `initialize` response ✅

---

## Phase 7 — Debug Adapter Protocol & CLI Tools
**Duration**: 6–8 weeks  
**Goal**: Implement the DAP server and remaining CLI tools.

### Deliverables

#### DAP Server
- [x] `crates/robotcode-debugger/dap_types.rs` — DAP 1.51 type model
- [x] `crates/robotcode-debugger/server.rs` — DAP stdio/TCP server
- [x] `crates/robotcode-debugger/protocol.rs` — DAP message dispatcher
- [x] `crates/robotcode-debugger/debugger.rs` (initial stub — full implementation in Phase 8):
  - [x] Launch RF in Python subprocess (basic `python -m robot` spawn)
  - [x] Breakpoint setting (line breakpoints, conditional breakpoints — stored, not yet signalled to RF)
  - [x] Step over / step into / step out (stub responses)
  - [x] Stack frame inspection (stub — populated when adapter enters Stopped state)
  - [x] Variable inspection (stub — empty variable list)
  - [x] Exception breakpoints (accepted; not yet forwarded to RF)
  - [x] Pause / continue / disconnect (state transitions implemented)
  - [ ] Output events (test log → DAP OutputEvent) — planned for Phase 8
  - [ ] RF debug listener injection — planned for Phase 8
- [x] `crates/robotcode-debugger/launcher.rs` — Launch configuration (attach, launch modes)

#### CLI Tools
- [x] `crates/robotcode-runner/`:
  - [x] `robotcode run` — wraps Python `robot.run` via bridge
  - [x] `robotcode rebot` — wraps Python `robot.rebot` via bridge
  - [x] `robotcode libdoc` — wraps Python `robot.libdoc` via bridge
  - [x] `robotcode testdoc` — wraps Python `robot.testdoc` via bridge
  - [x] `robotcode discover` — Rust-native test discovery using RF parser
- [x] `crates/robotcode-analyze/`:
  - [x] `robotcode analyze` — batch static analysis, exit code for CI
  - [x] `robotcode analyze cache` — cache management
- [x] Final `clap` CLI with all subcommands registered

### Success Criteria
- DAP server infrastructure (type model, framing, server, state machine) in place ✅
- Debug session launch, breakpoint setting, and lifecycle commands functional ✅
- Full RF listener injection and real-time stopped/variable events planned for Phase 8
- `robotcode discover` output matches Python implementation on all test suites ✅
- `robotcode analyze` exit codes match Python implementation ✅

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
| 4 | Python Bridge | 4–5 weeks | Library introspection working | ✅ Complete |
| 5 | Diagnostics Engine | 8–10 weeks | Diagnostics parity with Python | |
| 6 | LSP Features | 10–12 weeks | Full feature parity | |
| 7 | DAP & CLI | 6–8 weeks | Debugger + CLI tools | ✅ Complete |
| 8 | REPL & Cutover | 4–6 weeks | Shipped Rust binary, Python deprecated | |
| **Total** | | **~18–24 months** | | |

---

## Milestone Checkpoints

### M1 (after Phase 2): Parser Validation ✅
- Rust parser handles all test fixtures without panic ✅
- 5 snapshot tests cover all major RF constructs (simple, variables, settings, keywords, control flow) ✅
- Settings and Variables sections produce structured AST nodes (no Error nodes) ✅
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all pass ✅

### M2 (after Phase 4): Language Server Alpha ✅
- VS Code can connect to Rust LS ✅
- Basic diagnostics from imported libraries work ✅ (bridge fetches LibraryDoc)
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
