# 02 — Proposed Rust Architecture

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        VS Code / IntelliJ                            │
│         (TypeScript extension / LSP4IJ — unchanged protocol)         │
└───────────────────────────┬──────────────────────────────────────────┘
                            │ stdio / TCP (LSP 3.17 / DAP 1.51)
┌───────────────────────────▼──────────────────────────────────────────┐
│                    robotcode binary (Rust)                           │
│  ┌────────────┐  ┌────────────┐  ┌───────────┐  ┌────────────────┐  │
│  │  LSP server│  │ DAP server │  │  CLI tool │  │ Analyze tool   │  │
│  │ (tower-lsp)│  │ (custom)   │  │  (clap)   │  │ (batch mode)   │  │
│  └─────┬──────┘  └─────┬──────┘  └─────┬─────┘  └───────┬────────┘  │
│        │               │               │                 │           │
│  ┌─────▼───────────────▼───────────────▼─────────────────▼────────┐  │
│  │                   Core Analysis Engine                          │  │
│  │  ┌──────────────┐  ┌──────────────────────────────────────┐    │  │
│  │  │  RF Parser   │  │          Diagnostics Engine           │    │  │
│  │  │  (Rust-     │  │  ┌────────────────────────────────┐   │    │  │
│  │  │  native)    │  │  │ NamespaceAnalyzer              │   │    │  │
│  │  │             │  │  │ ImportResolver                 │   │    │  │
│  │  │  lexer      │  │  │ KeywordFinder                  │   │    │  │
│  │  │  parser     │  │  │ VariableScopeAnalyzer          │   │    │  │
│  │  │  AST types  │  │  │ DocumentCache                  │   │    │  │
│  │  └──────────────┘  │  └────────────────────────────────┘   │    │  │
│  │                     └──────────────────────────────────────┘    │  │
│  └────────────────────────────┬─────────────────────────────────────┘  │
│                               │  Python Bridge (library introspection) │
└───────────────────────────────┼──────────────────────────────────────┘
                                │
              ┌─────────────────▼──────────────────────┐
              │       Python subprocess / PyO3          │
              │  • robot.libdocpkg (keyword docs)       │
              │  • robot.running.arguments (arg specs)  │
              │  • robot.run / robot.rebot (execution)  │
              │  • Custom Python libraries in venv      │
              └────────────────────────────────────────┘
```

---

## Cargo Workspace Layout

```
robotcode/                          (repo root — existing)
├── Cargo.toml                      (workspace root, new)
├── Cargo.lock
├── crates/
│   ├── robotcode-core/             replaces packages/core
│   │   src/
│   │     lsp_types.rs              LSP 3.17 data model (generated or lsp-types crate)
│   │     text_document.rs          UTF-16 text with incremental edits
│   │     uri.rs                    URI utilities
│   │     workspace.rs              Multi-root workspace
│   │     documents_manager.rs      Open document registry
│   │     async_tools.rs            Cancellation, mutex
│   │     event.rs                  Event/observer
│   │     filewatcher.rs            notify-based file watching
│   │     utils/
│   │       logging.rs
│   │       dataclasses.rs          JSON serde helpers
│   │       path.rs                 File ID, same-file helpers
│   │
│   ├── robotcode-rf-parser/        new — replaces robot.parsing Python API usage
│   │   src/
│   │     lexer/
│   │       mod.rs
│   │       tokens.rs               RobotToken enum (mirrors robot.parsing.lexer.tokens)
│   │       scanner.rs              Hand-written or LALRPOP/Logos-based
│   │     parser/
│   │       mod.rs
│   │       ast.rs                  Complete AST (File, Section, Statement nodes)
│   │       visitor.rs              Visitor trait over AST
│   │     variables.rs              Variable pattern matching
│   │     escaping.rs               RF string escaping rules
│   │     versions.rs               RF 5/6/7 syntax quirks
│   │
│   ├── robotcode-jsonrpc2/         replaces packages/jsonrpc2
│   │   src/
│   │     protocol.rs               JSON-RPC 2.0 codec + dispatcher
│   │     server.rs                 stdio / TCP transport (tokio)
│   │
│   ├── robotcode-robot/            replaces packages/robot
│   │   src/
│   │     diagnostics/
│   │       library_doc.rs          LibraryDoc data model + Python bridge client
│   │       imports_manager.rs      Import caching + resolution
│   │       namespace_analyzer.rs   AST walk → scoped diagnostics
│   │       namespace.rs            Namespace data model
│   │       model_helper.rs         AST traversal helpers
│   │       keyword_finder.rs       Cross-namespace keyword lookup
│   │       import_resolver.rs      File-system import resolution
│   │       document_cache.rs       Per-document analysis cache
│   │       variable_scope.rs       Variable scoping rules
│   │       entities.rs             LibraryEntry, ResourceEntry, …
│   │       errors.rs               Diagnostic codes + messages
│   │     config/
│   │       model.rs                robot.toml config model (serde)
│   │     utils/
│   │       stubs.rs                Languages stub
│   │       visitor.rs              RF AST visitor bridge
│   │       match.rs                Normalized matching (case/space insensitive)
│   │
│   ├── robotcode-language-server/  replaces packages/language_server
│   │   src/
│   │     lib.rs                    tower-lsp LanguageServer impl
│   │     common/
│   │       diagnostics.rs
│   │       workspace.rs
│   │       documents.rs
│   │     robotframework/
│   │       completion.rs
│   │       hover.rs
│   │       goto.rs
│   │       references.rs
│   │       semantic_tokens.rs
│   │       formatting.rs
│   │       code_actions.rs
│   │       document_symbols.rs
│   │       rename.rs
│   │       inlay_hints.rs
│   │       folding_range.rs
│   │       signature_help.rs
│   │       code_lens.rs
│   │       highlight.rs
│   │
│   ├── robotcode-debugger/         replaces packages/debugger
│   │   src/
│   │     dap_types.rs              DAP 1.51 type model (serde)
│   │     protocol.rs               DAP message codec
│   │     server.rs                 DAP server (TCP/stdio)
│   │     debugger.rs               State machine + Python bridge
│   │     launcher/                 Launch configuration
│   │
│   ├── robotcode-analyze/          replaces packages/analyze
│   │   src/
│   │     cli.rs                    `robotcode analyze` subcommand
│   │     cache.rs                  On-disk analysis cache
│   │     config.rs                 Analysis-specific config
│   │
│   ├── robotcode-runner/           replaces packages/runner
│   │   src/
│   │     cli.rs                    `robotcode run/rebot/libdoc/testdoc` thin wrappers
│   │     discover.rs               Test discovery (uses Rust RF parser)
│   │
│   ├── robotcode-python-bridge/    new — Python interop layer
│   │   src/
│   │     bridge.rs                 Subprocess or PyO3 bridge trait
│   │     subprocess.rs             JSON-over-stdio protocol to Python helper
│   │     pyo3_impl.rs              Optional: direct PyO3 binding
│   │     library_introspector.rs   Calls libdocpkg, returns LibraryDoc
│   │     executor.rs               Calls robot.run / robot.rebot
│   │
│   └── robotcode/                  binary crate — entry point
│       src/
│         main.rs                   clap CLI, subcommand dispatch
│         hooks.rs                  Feature registration
│
├── python-bridge/                  new — Python helper scripts
│   helper.py                       JSON-over-stdio bridge server
│   library_introspector.py         wraps robot.libdocpkg
│   requirements.txt                robotframework (version-agnostic)
│
└── packages/                       existing — kept during transition
    (all existing Python packages remain until Phase 8)
```

---

## Key Crate Dependencies

```
robotcode-core
  └─ lsp-types, tokio, serde, serde_json, url, notify, log, tracing

robotcode-rf-parser
  └─ robotcode-core, logos (lexer), thiserror

robotcode-jsonrpc2
  └─ robotcode-core, tokio, serde_json, tower (optional)

robotcode-python-bridge
  └─ robotcode-core, tokio, serde_json, [pyo3 (optional feature)]

robotcode-robot
  └─ robotcode-core, robotcode-rf-parser, robotcode-python-bridge

robotcode-language-server
  └─ robotcode-core, robotcode-robot, robotcode-rf-parser, tower-lsp

robotcode-debugger
  └─ robotcode-core, robotcode-python-bridge, tokio, serde_json

robotcode-analyze
  └─ robotcode-core, robotcode-robot, clap

robotcode-runner
  └─ robotcode-core, robotcode-python-bridge, clap

robotcode (binary)
  └─ all above crates, clap
```

---

## Key Third-Party Rust Crates

| Rust Crate | Purpose | Replaces |
|-----------|---------|---------|
| `tower-lsp` | LSP server framework | Hand-rolled jsonrpc2 + language_server |
| `lsp-types` | LSP 3.17 data types | `core/lsp/types.py` (7,407 lines!) |
| `logos` | Lexer generator (for RF tokenizer) | `robot.parsing.lexer` |
| `tokio` | Async runtime | Python asyncio |
| `serde` / `serde_json` | JSON serialization | Custom dataclasses helpers |
| `clap` | CLI argument parsing | Click + pluggy |
| `notify` | File system watching | Python watchdog/inotify |
| `pyo3` | Python-Rust FFI (optional) | Python subprocess bridge |
| `tracing` | Structured logging | Python logging |
| `thiserror` / `anyhow` | Error handling | Python exceptions |
| `dashmap` | Concurrent hashmap | Python dicts + asyncio locks |
| `ropey` | Efficient text rope (incremental edits) | Custom text_document.py |
| `regex` | Regex (embedded arguments) | Python re |
| `toml` | robot.toml parsing | Python tomllib |
| `url` | URI handling | Custom uri.py |
| `parking_lot` | Fast mutexes | asyncio.Lock |
| `rayon` | Data parallelism (batch analysis) | concurrent.futures |

---

## Critical Design Decisions

### 1. Robot Framework Parser Strategy

**Option A — Hand-written Rust parser** (recommended):
- Full control over error recovery (IDE parsers need to handle broken files)
- No grammar DSL dependency
- Can exactly mirror RF's parsing quirks across versions 5/6/7
- Precedent: rust-analyzer's hand-written Rust parser

**Option B — Logos lexer + hand-written recursive-descent parser**:
- Logos generates a fast DFA-based lexer from token regex annotations
- Combine with hand-written recursive descent for the parser
- Best of both worlds: fast tokenization, flexible parsing

**Option C — tree-sitter grammar**:
- Existing community `tree-sitter-robotframework` grammar exists
- Mature incremental parsing
- Downside: C library dependency, grammar may lag behind RF syntax

**Recommendation**: Option B (Logos + recursive descent). Logos is extremely fast (~GB/s throughput), and recursive descent is easy to extend for error recovery.

### 2. Python Library Introspection Bridge

Robot Framework Python libraries (SeleniumLibrary, RequestsLibrary, etc.) are loaded via `importlib` in the user's Python environment. We cannot replace this in Rust.

**Strategy**:
- `robotcode-python-bridge` crate spawns a long-lived Python subprocess per workspace folder
- Communication: newline-delimited JSON over stdio (similar to existing pattern in test discovery)
- The Python helper (`python-bridge/helper.py`) uses `robot.libdocpkg.LibraryDocumentation` to extract keyword signatures, documentation, argument specs
- Results are cached in the Rust side (invalidated by file mtime)
- **PyO3 as optional feature**: For distribution in the VS Code bundled extension, PyO3 embedding avoids subprocess overhead; for CLI use, subprocess is simpler

### 3. Async Runtime

- Use **tokio** with multi-threaded runtime for the LSP server
- Document analysis runs on tokio's blocking thread pool (CPU-intensive)
- File watching uses `notify` integrated into tokio
- Cancellation via `tokio_util::CancellationToken` (mirrors Python asyncio cancellation)

### 4. Text Document Representation

- Use **ropey** rope data structure for O(log n) incremental text edits
- LSP uses UTF-16 code unit offsets — ropey supports this natively
- `TextDocument` struct wraps ropey + version counter + URI

### 5. Configuration (robot.toml)

- Parse `robot.toml` with the `toml` crate into a strongly-typed `RobotConfig` struct
- Mirror the existing `config/model.py` structure exactly for compatibility
- Support all existing profile/inheritance semantics

### 6. Semantic Token Legend

- The semantic token legend (token types and modifiers) must remain **identical** to the current Python implementation so existing VS Code color themes work
- Define the legend as a compile-time constant; validate against snapshot tests

### 7. Diagnostics Source Names

- All diagnostic `source` strings (e.g., `"robotcode"`, `"robotcode (warning)"`) must remain identical
- User `.editorconfig` / suppression rules depend on these strings

### 8. Multi-Version RF Support

- The Rust RF parser must correctly handle syntax changes between RF 5.0, 6.0, and 7.0
- Use a version enum (`RfVersion`) threaded through parser calls
- The Python bridge reports the installed RF version; the Rust parser uses this to select syntax rules

---

## Python Bridge Protocol

The `python-bridge/helper.py` subprocess exposes a simple JSON-over-stdio protocol:

```jsonc
// Request (Rust → Python)
{"id": 1, "method": "library_doc", "params": {"name": "SeleniumLibrary", "args": [], "base_dir": "/workspace"}}

// Response (Python → Rust)
{"id": 1, "result": {"name": "SeleniumLibrary", "keywords": [...], "version": "6.1.3", ...}}

// Error response
{"id": 1, "error": {"code": -32000, "message": "Library not found: SeleniumLibrary"}}
```

Methods exposed by the Python bridge:
| Method | Purpose |
|--------|---------|
| `library_doc` | Get keyword docs for a Python library |
| `variables_doc` | Get variable file contents |
| `embedded_args` | Parse embedded argument patterns |
| `normalize` | RF normalized string comparison |
| `rf_version` | Get installed RF version string |
| `run` | Execute `robot.run()` (runner integration) |
| `discover` | Discover tests using `TestSuiteBuilder` |

---

## VS Code Extension Changes

Minimal changes required:

1. **`languageclientsmanger.ts`**: Add logic to detect and prefer the Rust binary (`robotcode-lsp`) over the Python-based language server. Fall back to Python if Rust binary not found.
2. **`pythonmanger.ts`**: Pass the selected Python interpreter path to the Rust binary via `--python` flag (for the bridge subprocess).
3. **`debugmanager.ts`**: Update launch args to use Rust DAP server binary.
4. **`package.json`**: Add `robotcode-lsp` binary to bundled resources.

No changes to extension activation logic, command contributions, or configuration schema.

---

## IntelliJ Plugin Changes

Update `build.gradle.kts` server launch command from:
```kotlin
listOf("python", "-m", "robotcode", "language-server")
```
to:
```kotlin
listOf("robotcode-lsp", "--python", pythonInterpreterPath)
```

No changes to LSP4IJ configuration or plugin manifest.
