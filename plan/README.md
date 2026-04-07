# RobotCode → Rust Migration Plan

This directory contains the detailed migration plan for rewriting RobotCode's Python packages to Rust while maintaining full feature parity and improving performance.

## Documents

| File | Description |
|------|-------------|
| [01-analysis.md](01-analysis.md) | Current codebase analysis — packages, dependencies, Robot Framework API usage |
| [02-architecture.md](02-architecture.md) | Proposed Rust architecture — crate layout, key design decisions |
| [03-phases.md](03-phases.md) | Phased migration roadmap with milestones and success criteria |
| [04-rf-python-bridge.md](04-rf-python-bridge.md) | Strategy for bridging Robot Framework's Python APIs during and after migration |
| [05-performance.md](05-performance.md) | Performance improvement opportunities and benchmarking strategy |
| [06-risks.md](06-risks.md) | Risk register, mitigation strategies, and go/no-go criteria |

## TL;DR

The migration is organized into **8 phases** spanning approximately 18–24 months of engineering effort:

1. **Foundation** — Cargo workspace, core data types, LSP/DAP protocol types
2. **Robot Framework Parser** — Rust-native `.robot`/`.resource` lexer + AST parser
3. **JSON-RPC 2.0 & LSP Transport** — Async server infrastructure
4. **Diagnostics Engine** — Namespace analyzer, import resolver, variable scope
5. **LSP Features** — Completion, hover, go-to-definition, semantic tokens, formatting
6. **Debug Adapter Protocol** — DAP server wrapping Python RF execution subprocess
7. **CLI, Configuration & Analysis** — `robotcode` CLI, `robot.toml` config, static analysis
8. **REPL & Final Cutover** — Interactive shell, bundle, VS Code/IntelliJ integration

The Python Robot Framework runtime (for **execution**) is never fully replaced — a Python subprocess bridge is maintained because RF's keyword/library ecosystem is irreplaceable Python. All **analysis, parsing, LSP intelligence, and tooling** move to Rust.

## Key Design Decisions

- **PyO3 / subprocess bridge**: Python RF APIs for library introspection are called from Rust via an embedded or subprocess Python interpreter.
- **tower-lsp**: The de-facto Rust LSP framework, replaces the hand-rolled `jsonrpc2` + `language_server` Python stack.
- **tree-sitter or hand-written parser**: A Rust-native Robot Framework parser replaces use of `robot.parsing`.
- **Dual-mode shipping**: During transition, the Rust binary coexists with the Python packages; the VS Code extension picks whichever is available.
