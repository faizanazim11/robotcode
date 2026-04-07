# 01 — Current Codebase Analysis

## Repository Overview

| Property | Value |
|----------|-------|
| Language | Python 3.10–3.14 |
| Total Python LOC (packages) | ~57,600 |
| Total Python LOC (CLI + tests) | ~9,800 |
| Number of packages | 11 |
| Existing Rust code | None |
| Build system | Hatch + hatchling |
| Test framework | pytest + regtest2 snapshots |
| Test matrix | Python 3.10–3.14 × Robot Framework 5.0–7.4 |

---

## Package Inventory

### `robotcode-core` — 11,707 lines

**Purpose**: Shared utilities and protocol data model used by all other packages.

**Key modules**:
- `lsp/types.py` (7,407 lines) — Auto-generated LSP 3.17 type model (Position, Range, Diagnostic, CompletionItem, …)
- `async_tools.py` (444 lines) — Asyncio concurrency utilities (mutex, cancellation tokens, task groups)
- `text_document.py` — In-memory text document with incremental update support
- `uri.py` — URI encoding/decoding
- `utils/dataclasses.py` (724 lines) — JSON serialisation helpers
- `utils/logging.py` (634 lines) — Structured logging
- `documents_manager.py` — Workspace document registry
- `event.py` — Event/observer pattern
- `filewatcher.py` — File system change notifications
- `workspace.py` — Multi-root workspace abstraction

**External dependencies**: `typing-extensions`

**Migration priority**: HIGH — everything depends on it; Rust equivalents are idiomatic.

---

### `robotcode-plugin` — 923 lines

**Purpose**: Plugin discovery and CLI entry-point system.

**Key modules**:
- Plugin manager wrapping **pluggy** (hookspecs for `register_cli_commands`, `register_tool_config_classes`)
- CLI root command using **Click**
- Hook specs that each package implements to register its commands

**External dependencies**: `click>=8.2`, `pluggy>=1.0`, `tomli`/`tomllib`

**Migration priority**: MEDIUM — replaced by Clap-based CLI with dynamic dispatch or compile-time feature flags.

---

### `robotcode-jsonrpc2` — 1,417 lines

**Purpose**: Asynchronous JSON-RPC 2.0 server, base for both LSP and DAP.

**Key modules**:
- `protocol.py` (1,010 lines) — Full duplex JSON-RPC 2.0 message dispatcher; method registration via `@rpc_method` descriptor
- `server.py` — TCP / stdio transports

**External dependencies**: `robotcode-core`

**Migration priority**: HIGH — replaced by **tower-lsp** (LSP) and a lightweight custom DAP codec in Rust.

---

### `robotcode-robot` — 16,586 lines

**Purpose**: Robot Framework project model, diagnostics engine, and library introspection. The largest and most Robot-Framework-coupled package.

**Key modules**:
- `diagnostics/library_doc.py` (3,303 lines) — Introspects Python keyword libraries, generates `LibraryDoc` model
- `diagnostics/imports_manager.py` (2,015 lines) — Caches and resolves `Library`, `Resource`, `Variables` imports
- `diagnostics/namespace_analyzer.py` (1,999 lines) — Walks RF AST, builds scoped keyword/variable namespaces, emits diagnostics
- `diagnostics/namespace.py` (806 lines) — Namespace data model
- `diagnostics/model_helper.py` (790 lines) — AST traversal helpers
- `diagnostics/keyword_finder.py` (526 lines) — Finds keyword definitions across namespaces
- `diagnostics/import_resolver.py` (656 lines) — Resolves import paths relative to workspace root
- `diagnostics/document_cache_helper.py` (697 lines) — Document-level analysis cache
- `diagnostics/variable_scope.py` — Variable resolution with RF scoping rules
- `config/model.py` (2,600 lines) — `robot.toml` configuration model
- `utils/variables.py` — Variable pattern matching/parsing
- `utils/visitor.py` — Visitor pattern over RF AST nodes

**Robot Framework APIs used** (critical, see section below):
- `robot.parsing.lexer.tokens` — Token types
- `robot.parsing.model.blocks` — File, Section, TestCase, Keyword, …
- `robot.parsing.model.statements` — All statement node types
- `robot.running.arguments` — ArgumentSpec, EmbeddedArguments
- `robot.libdocpkg` — LibraryDocumentation builder
- `robot.variables.search` — Variable pattern matching
- `robot.utils.escaping` — String unescaping

**Migration priority**: CRITICAL — core of the performance problem; largest block of Robot-Framework-coupled code.

---

### `robotcode-language-server` — 16,104 lines

**Purpose**: Full LSP 3.17 implementation for Robot Framework.

**Structure**: Two layers:
1. `common/` — Language-agnostic LSP protocol parts (diagnostics, completion, hover, workspace, etc.)
2. `robotframework/parts/` — RF-specific implementations of each LSP method

**LSP features implemented**:
- Text document sync (open/close/change/save)
- Diagnostics (push model)
- Completion (2,588 lines — keywords, variables, settings, library names)
- Hover (keyword/variable documentation)
- Go-to-definition / declaration / implementation
- Find references
- Document symbols / workspace symbols
- Semantic tokens (1,689 lines — full token type/modifier mapping)
- Code actions (quick fixes + refactorings)
- Code lens
- Rename (symbol renaming)
- Inlay hints / inline values
- Folding ranges
- Selection ranges
- Document formatting
- Signature help
- Document highlight
- Linked editing ranges
- HTTP documentation server

**External dependencies**: `robotcode-jsonrpc2`, `robotcode-robot`, `robotcode-analyze`

**Migration priority**: CRITICAL — primary user-facing product.

---

### `robotcode-analyze` — 1,834 lines

**Purpose**: Standalone static analysis CLI tool (batch mode, CI integration).

**Key modules**:
- `code/cli.py` (500 lines) — `robotcode analyze` subcommand
- `cache/cli.py` (421 lines) — Analysis result caching
- `config.py` (410 lines) — Analysis configuration

**Migration priority**: MEDIUM — depends on diagnostics engine.

---

### `robotcode-debugger` — 5,893 lines

**Purpose**: Debug Adapter Protocol (DAP) server for Robot Framework execution.

**Key modules**:
- `debugger.py` (2,241 lines) — Core debug state machine (breakpoints, stepping, variable inspection, stack frames)
- `dap_types.py` (1,183 lines) — Complete DAP 1.51 type model
- `listeners.py` (443 lines) — RF execution event listeners
- `server.py` (421 lines) — DAP transport
- `protocol.py` — DAP message dispatcher
- `launcher/` — Launch configuration handling

**Robot Framework APIs used**:
- `robot.running.EXECUTION_CONTEXTS` — Access to live execution state
- `robot.api.SuiteVisitor` — Suite/test/keyword tree walking
- `robot.model` — TestSuite, TestCase, Keyword models
- `robot.reporting.ResultWriter` — Output XML/HTML generation

**Migration priority**: MEDIUM — the actual RF execution subprocess must remain Python; Rust handles the DAP protocol layer.

---

### `robotcode-runner` — 1,791 lines

**Purpose**: Enhanced `robot`, `rebot`, `libdoc`, `testdoc` CLI wrappers + test discovery.

**Key modules**:
- `cli/discover/discover.py` (1,092 lines) — Test discovery using RF's `TestSuiteBuilder`
- `cli/robot.py` — Wrapped `robot` command
- `cli/rebot.py` — Wrapped `rebot` command
- `cli/libdoc.py` — Wrapped `libdoc` command

**Robot Framework APIs used**:
- `robot.run.RobotFramework`
- `robot.running.builder.TestSuiteBuilder`
- `robot.running.builder.builders.SuiteStructureParser`
- `robot.model.ModelModifier`, `SuiteVisitor`

**Migration priority**: LOW — thin wrappers; test discovery moves to Rust parser.

---

### `robotcode-modifiers` — 65 lines

**Purpose**: `SuiteModifier` plugins for RF execution (long-name normalisation).

**Migration priority**: LOW — minimal code, remains Python as it hooks into RF execution.

---

### `robotcode-repl` — 688 lines

**Purpose**: Interactive Robot Framework REPL (read-eval-print loop).

**Key modules**:
- `base_interpreter.py` — Keyword evaluation over stdin
- `console_interpreter.py` — Terminal UI

**Migration priority**: LOW — execution is Python-bound; only transport/protocol layer can move to Rust.

---

### `robotcode-repl-server` — 590 lines

**Purpose**: Remote REPL server (JSON-RPC 2.0 transport for the REPL).

**Migration priority**: LOW — same as `repl`.

---

## Robot Framework Python API Dependency Map

These are the RF internal APIs currently consumed. They define the boundaries of the Python bridge:

### Parser / AST APIs (replaceable with Rust parser)
```
robot.parsing.lexer.tokens          Token types and constants
robot.parsing.model.blocks          File, Section, TestCase, Keyword, VariableSection, …
robot.parsing.model.statements      All 40+ statement node types
robot.parsing.lexer.settings        Settings token helpers
robot.api.parsing.get_model         Parse a .robot file to AST
robot.api.get_model                 Parse a .robot file to AST (older)
```

### Running / Argument Model (partially replaceable)
```
robot.running.arguments.argumentspec    ArgInfo, ArgumentSpec
robot.running.arguments.argumentresolver  NamedArgumentResolver
robot.running.arguments.embedded        EmbeddedArguments (regex keyword matching)
robot.running.builder.TestSuiteBuilder  Suite construction from files
robot.running.EXECUTION_CONTEXTS        Live execution state (debugger only)
```

### Library Documentation (Python bridge required)
```
robot.libdocpkg.LibraryDocumentation    Build docs from Python library class
robot.libdocpkg.robotbuilder.KeywordDocBuilder
robot.libdocpkg.htmlwriter.LibdocHtmlWriter
robot.libraries.STDLIBS                 List of built-in library names
```

### Output / Execution (Python bridge required)
```
robot.run.RobotFramework                Execute RF
robot.rebot.Rebot
robot.reporting.ResultWriter
robot.output.LOGGER
robot.running.EXECUTION_CONTEXTS
```

### Variables / Utilities (replaceable in Rust)
```
robot.variables.search.*               Variable pattern detection
robot.utils.escaping.unescape          RF string escaping rules
robot.utils.NormalizedDict/normalize   Case/space-insensitive matching
robot.errors.*                         Error type hierarchy
```

---

## VS Code Extension (TypeScript)

**Location**: `vscode-client/extension/`  
**Size**: ~2,000 lines TypeScript  
**Build**: esbuild  

Key managers:
- `languageclientsmanger.ts` — Starts/manages language server process(es)
- `testcontrollermanager.ts` — VS Code Test Explorer integration
- `debugmanager.ts` — DAP client lifecycle
- `pythonmanger.ts` — Python interpreter/venv detection

The VS Code extension communicates with the language server over stdio/TCP — it is language-server-agnostic. After migration, only `languageclientsmanger.ts` and `pythonmanger.ts` need changes to launch the Rust binary instead of a Python process.

---

## IntelliJ Plugin (Kotlin)

**Location**: `intellij-client/`  
**Integration**: LSP4IJ bridge  
**Change required**: Update server launch command to use Rust binary; no protocol changes.
