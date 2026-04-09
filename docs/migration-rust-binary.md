# Migrating to the Rust Binary

> **Status**: Available from RobotCode Phase 8 onward.

From Phase 8, RobotCode ships a native **Rust binary** (`robotcode`) for Linux,
macOS, and Windows alongside the existing Python packages. The Rust binary
provides significantly better startup performance and eliminates the need to
install the Python language-server packages in your project environment.

---

## What changed

| Component | Before Phase 8 | Phase 8 onward |
|-----------|---------------|----------------|
| Language Server | `python -m robotcode language-server` | `robotcode language-server --python <python>` |
| Debug Adapter | `python -m robotcode debug` | `robotcode debug --python <python>` |
| REPL Server | `python -m robotcode repl-server` | `robotcode repl [--python <python>]` |
| CLI tools | `python -m robotcode run/rebot/…` | `robotcode run/rebot/…` |

The `--python` flag tells the Rust binary which Python interpreter to use for
Robot Framework itself (library loading, test execution). You still need Python
and Robot Framework installed in your project — the Rust binary replaces only
the language-server and debug-adapter processes.

---

## VS Code extension

The VS Code extension detects the bundled Rust binary automatically. No
configuration change is required. If the binary is not present (e.g. you are
running the extension from source), it falls back to the Python-based launcher.

To verify which launcher is being used, open the *RobotCode* output channel and
look for one of:

```
Using Rust language server binary: /path/to/bundled/bin/robotcode
```

or

```
Starting debug launcher with Rust binary: /path/to/bundled/bin/robotcode
```

---

## IntelliJ plugin

The IntelliJ plugin similarly detects the bundled binary in
`<plugin-data>/bundled/bin/robotcode` and uses it automatically, passing
`--python <sdk>` for the active project SDK.

---

## Manual / headless usage

If you invoke `robotcode` directly (CI, Neovim, other LSP clients), replace:

```bash
# Before
python -m robotcode language-server --stdio

# After (Rust binary on PATH, or use the full path)
robotcode language-server --stdio --python $(which python)
```

The `--python` argument is optional if `python3` is on the system PATH. The
bridge will default to `python3` when not specified.

---

## Running the REPL server

```bash
# stdio (default)
robotcode repl --python /usr/bin/python3

# TCP
robotcode repl --tcp 7700 --python /usr/bin/python3
```

The REPL server speaks JSON-RPC 2.0 over newline-delimited streams.

**Supported methods:**

| Method | Description |
|--------|-------------|
| `evaluate` | Run a keyword call; returns `result`, `log`, `error` |
| `history` | Return the list of past evaluations |
| `history/clear` | Clear the session history |
| `complete` | Prefix-based completion from history |
| `shutdown` | Gracefully stop the server |

---

## Python packages — deprecation timeline

| Milestone | Action |
|-----------|--------|
| Phase 8 (now) | Python language-server and debugger emit `DeprecationWarning` at startup |
| 6 months after GA | Python packages marked as `Deprecated` on PyPI |
| 12 months after GA | Python language-server / debugger packages archived; no further updates |

The CLI tools (`robotcode run`, `robotcode rebot`, `robotcode libdoc`,
`robotcode testdoc`, `robotcode discover`, `robotcode analyze`) are already
implemented natively in the Rust binary and remain available via the Python CLI
as thin wrappers until the packages are retired.

---

## Cross-compilation / building from source

The Rust binary is built for the following targets:

| Target | Platform |
|--------|----------|
| `x86_64-unknown-linux-gnu` | Linux x86-64 |
| `aarch64-unknown-linux-gnu` | Linux ARM64 |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows x86-64 |

To build locally:

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the binary
cargo build --release -p robotcode

# Binary is at: target/release/robotcode (or robotcode.exe on Windows)
```

---

## Getting help

- [GitHub Discussions](https://github.com/robotcodedev/robotcode/discussions)
- [GitHub Issues](https://github.com/robotcodedev/robotcode/issues)
- [CHANGELOG](../CHANGELOG.md)
