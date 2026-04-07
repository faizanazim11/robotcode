# 04 — Robot Framework Python Bridge Strategy

## The Core Problem

RobotCode's analysis engine is deeply entangled with Robot Framework's Python internals. Specifically:

1. **Library introspection** — To provide completions and type checking for `SeleniumLibrary`, `RequestsLibrary`, etc., RobotCode must import those libraries into a Python process and call `robot.libdocpkg.LibraryDocumentation` to extract keyword signatures. This requires the actual Python library to be importable in the user's virtual environment.

2. **Argument spec parsing** — RF argument types (named arguments, embedded arguments, varargs, kwargs, positional-only) are modeled in `robot.running.arguments.argumentspec`. The embedded argument pattern (`${value:\w+}`) uses Python regex.

3. **Variables files** — `.py` variables files define variables as Python module-level attributes; they must be executed in Python to get their values.

4. **Test execution** — `robot.run`, `robot.rebot` must remain Python — there is no path to rewriting the RF executor in Rust.

5. **Debugger listeners** — RF execution lifecycle hooks (`robot.api.SuiteVisitor`, `robot.running.EXECUTION_CONTEXTS`) are Python-only.

## What Does NOT Require Python

The following currently use Robot Framework Python APIs but can be **fully replaced by Rust**:

| Current Python API | Rust Replacement | Notes |
|---|---|---|
| `robot.api.parsing.get_model()` | Rust RF parser | Core of Phase 2 |
| `robot.parsing.lexer.tokens.Token` | `RobotToken` enum in Rust | 1:1 mapping |
| `robot.parsing.model.blocks.*` | Rust AST block nodes | Structural equivalents |
| `robot.parsing.model.statements.*` | Rust AST statement nodes | All 40+ types |
| `robot.variables.search.*` | `variables.rs` in Rust | Regex-based, portable |
| `robot.utils.escaping.unescape` | `escaping.rs` in Rust | Simple string transforms |
| `robot.utils.normalize` / `NormalizedDict` | `utils/match.rs` in Rust | Case+space insensitive |
| `robot.errors.*` (diagnostic classification) | `errors.rs` in Rust | Error code constants |

---

## Bridge Deployment Models

Two deployment models exist, selectable at compile time or runtime:

### Model A: Subprocess Bridge (Default)

```
┌─────────────────────────────────┐
│   robotcode (Rust binary)       │
│                                 │
│  ┌───────────────────────────┐  │
│  │  SubprocessBridge         │  │    stdin/stdout
│  │  (one per workspace root) │◄─┼──────────────────►  python helper.py
│  └───────────────────────────┘  │                      (in user's venv)
└─────────────────────────────────┘
```

**Lifecycle**:
1. When the language server activates for a workspace folder, `SubprocessBridge::start()` spawns `<python> python-bridge/helper.py`
2. The Python interpreter used is the one detected by `pythonmanger.ts` (VS Code's Python extension), passed as `--python /path/to/python`
3. The bridge is long-lived — it stays running for the VS Code session
4. If the bridge crashes (e.g., Python exception), it is restarted automatically with a 1-second backoff
5. On workspace close, `SubprocessBridge::stop()` sends a `shutdown` request

**Communication**: Newline-delimited JSON (NDJSON) over stdio:
```
stdin  → Rust sends request  {"id":1,"method":"library_doc","params":{...}}\n
stdout ← Python sends response {"id":1,"result":{...}}\n
stderr ← Python sends diagnostic log lines (shown in Output panel)
```

**Timeout handling**: Each bridge call has a 30-second timeout; library loading is deferred if bridge is not yet ready.

**Advantages**:
- Works with any Python version ≥ 3.8
- No build-time dependency on Python
- Completely isolated from the Rust process memory
- User can change Python interpreter without restarting the Rust binary

**Disadvantages**:
- ~50ms subprocess startup cost (amortized)
- 1–5ms IPC latency per library load (acceptable with caching)

---

### Model B: PyO3 Embedded Python (Optional Feature)

```
┌─────────────────────────────────────────┐
│   robotcode (Rust binary)               │
│                                         │
│  ┌──────────────────────────────────┐   │
│  │  Pyo3Bridge                      │   │
│  │  Python::with_gil(|py| { ... })  │   │
│  └──────────────────────────────────┘   │
│         ↓                               │
│  ┌──────────────────────────────────┐   │
│  │  Embedded CPython interpreter    │   │
│  │  (sys.path set to user's venv)   │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

**Activation**: Compile with `--features pyo3-bridge`

**Advantages**:
- Zero subprocess overhead
- Sub-millisecond library introspection after first load
- Tighter integration (can call any Python API directly)

**Disadvantages**:
- Build-time Python development headers required
- Binary is tied to a specific CPython ABI version
- GIL contention if multiple threads need Python access
- Harder to switch Python interpreters at runtime

**Recommendation**: Use as an opt-in feature for power users and CI scenarios; default to subprocess bridge for distribution.

---

## Python Bridge API Contract

### `helper.py` Request/Response Format

```python
# helper.py message loop
import json, sys

for line in sys.stdin:
    req = json.loads(line)
    try:
        result = dispatch(req["method"], req["params"])
        print(json.dumps({"id": req["id"], "result": result}), flush=True)
    except Exception as e:
        print(json.dumps({"id": req["id"], "error": {"code": -32000, "message": str(e)}}), flush=True)
```

### Method Specifications

#### `library_doc`
Introspect a Robot Framework keyword library.

**Request params**:
```json
{
  "name": "SeleniumLibrary",
  "args": ["timeout=10s"],
  "base_dir": "/workspace/tests",
  "python_path": ["/workspace/src"],
  "variables": {"BROWSER": "chrome"}
}
```

**Response result** (abridged):
```json
{
  "name": "SeleniumLibrary",
  "doc": "SeleniumLibrary is a web testing library...",
  "version": "6.1.3",
  "scope": "SUITE",
  "named_args": true,
  "keywords": [
    {
      "name": "Open Browser",
      "args": [
        {"name": "url", "kind": "POSITIONAL_OR_NAMED", "default": null, "types": ["str"]},
        {"name": "browser", "kind": "POSITIONAL_OR_NAMED", "default": "firefox", "types": ["str"]}
      ],
      "doc": "Opens a new browser instance...",
      "tags": [],
      "source": "/path/to/seleniumlibrary/keywords/_browsermanagement.py",
      "lineno": 42
    }
  ],
  "inits": [],
  "typedocs": []
}
```

#### `variables_doc`
Load a Robot Framework variables file.

**Request params**:
```json
{
  "path": "/workspace/variables/common.py",
  "args": [],
  "base_dir": "/workspace"
}
```

**Response result**:
```json
{
  "variables": [
    {"name": "${BASE_URL}", "value": "https://example.com", "source": "/workspace/variables/common.py", "lineno": 1}
  ]
}
```

#### `embedded_args`
Parse an embedded argument pattern from a keyword name.

**Request params**:
```json
{"pattern": "the user ${name} logs in with password ${password}"}
```

**Response result**:
```json
{
  "name": "the user ${name} logs in with password ${password}",
  "args": ["name", "password"],
  "regex": "^the user (.+) logs in with password (.+)$"
}
```

#### `normalize`
RF-style normalized comparison (lowercase, remove spaces/underscores).

**Request params**:
```json
{"value": "My Keyword Name", "remove_underscores": true}
```

**Response result**:
```json
{"normalized": "mykeywordname"}
```

#### `rf_version`
Get the installed Robot Framework version.

**Request params**: `{}`

**Response result**:
```json
{"version": "7.1.1", "major": 7, "minor": 1, "patch": 1}
```

#### `discover`
Discover all tests in a workspace using RF's TestSuiteBuilder.

**Request params**:
```json
{
  "paths": ["/workspace/tests"],
  "include_tags": ["smoke"],
  "exclude_tags": ["wip"],
  "python_path": ["/workspace/src"]
}
```

**Response result**:
```json
{
  "suites": [
    {
      "name": "Login Tests",
      "source": "/workspace/tests/login.robot",
      "tests": [
        {"name": "Valid Login", "tags": ["smoke"], "lineno": 5}
      ]
    }
  ]
}
```

---

## Caching Strategy

Library introspection is expensive (100–2000ms per library). The Rust side must cache aggressively:

### Cache Keys
```rust
struct LibraryCacheKey {
    name: String,                  // Library name or path
    args: Vec<String>,             // Constructor arguments
    python_path: Vec<PathBuf>,     // sys.path additions
    python_interpreter: PathBuf,   // Which Python binary
    mtime: Option<SystemTime>,     // Source file modification time (for .py libs)
}
```

### Cache Invalidation Triggers
- File system event for the library's source `.py` file
- Workspace Python interpreter change (user switches venv)
- `robot.toml` `python-path` change
- Manual "Reload Libraries" command

### Persistent Cache (Disk)
- Cache serialized `LibraryDoc` structs to `~/.cache/robotcode/<hash>.cbor`
- Hash is SHA-256 of (library_name + version + python_path)
- Enables fast startup: no bridge call needed if disk cache is fresh
- Cache entries expire after 7 days or when library file mtime changes

---

## Handling Multiple Python Versions / Virtual Environments

The language server supports multiple workspace folders, each potentially using a different Python interpreter:

```rust
struct WorkspaceBridge {
    folder: WorkspaceFolder,
    python_interpreter: PathBuf,
    bridge: Arc<SubprocessBridge>,
    cache: Arc<LibraryCache>,
}
```

- One `SubprocessBridge` instance per workspace folder
- The Python interpreter is discovered via VS Code's Python extension API (`ms-python.python`) or `pythonmanger.ts`
- If no interpreter is found, fallback to `python3` on PATH
- The Rust binary reads `--python /path/to/python` from launch args (set by VS Code extension)

---

## venv Site-Packages Scanning

For library name completion (typing `Library    Sel<TAB>`), the Rust side scans the user's venv for installable RF libraries without going through the Python bridge:

```rust
async fn scan_venv_for_libraries(python: &Path) -> Vec<String> {
    // Run: python -c "import site; print(site.getsitepackages())"
    // Then scan each site-packages dir for robot_framework_*.dist-info or
    // directories containing __init__.py + keywords that register as RF libs
}
```

This avoids expensive bridge calls for completion — we only call the bridge when the user selects a completion item (to get the full keyword list).

---

## Testing the Bridge

### Unit Tests (Rust side)
- Use `MockBridge` that returns hardcoded `LibraryDoc` JSON
- Tests do not require Python to be installed

### Integration Tests
- Require Python 3.10+ and `robotframework` installed
- Run in CI via `hatch run test` matrix
- Test all `helper.py` methods against real RF libraries
- Snapshot-test `LibraryDoc` output for standard libraries across RF 5.0–7.4

### Fuzzing
- Fuzz the NDJSON parser in `SubprocessBridge` to prevent parser panics on malformed Python output
