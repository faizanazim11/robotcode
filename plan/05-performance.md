# 05 — Performance Improvement Opportunities

## Current Python Performance Characteristics

Based on the codebase analysis, the key performance bottlenecks in the current Python implementation are:

| Component | Why It's Slow | Typical Latency |
|-----------|--------------|----------------|
| RF file parsing (`robot.parsing`) | Python bytecode, single-threaded | 10–50ms per file |
| Library introspection (`libdocpkg`) | `importlib` + reflection, re-runs every session | 100ms–2s per library |
| Namespace analysis | Pure Python AST walk, asyncio concurrency | 20–100ms per file |
| Workspace-wide analysis (500 files) | Serial analysis with asyncio | 10–30 seconds |
| Semantic token generation | Python string manipulation | 5–20ms per file |
| Completion computation | Linear keyword search across all namespaces | 50–200ms |
| JSON serialization (LSP messages) | Python `json` module, custom `dataclasses` | 2–10ms per message |
| File watching | Python `watchdog`, polling fallback on some platforms | 100ms–1s latency |
| Import resolution (disk I/O) | Python `os.path`, no parallelism | 5–50ms per import |

---

## Rust Performance Targets

| Component | Python Baseline | Rust Target | Improvement Factor |
|-----------|----------------|-------------|-------------------|
| RF file parse (100-line file) | ~15ms | ~0.2ms | 75× |
| RF file parse (1000-line file) | ~80ms | ~1ms | 80× |
| Workspace initial analysis (500 files) | ~20s | ~0.5s | 40× |
| Incremental re-analysis (single file change) | ~500ms | ~5ms | 100× |
| Semantic tokens (1000-line file) | ~15ms | ~0.5ms | 30× |
| Completion request | ~150ms | ~5ms | 30× |
| Hover request | ~50ms | ~2ms | 25× |
| Find references (workspace) | ~3s | ~100ms | 30× |
| JSON serialization (large message) | ~5ms | ~0.1ms | 50× |
| File watching event delivery | ~200ms | ~10ms | 20× |
| Library introspection (cached) | ~1ms (in-process) | ~1ms (cached disk) | 1× |
| Library introspection (bridge, cold) | ~500ms | ~500ms | 1× (bridge latency) |
| Library introspection (bridge, warm) | ~150ms | ~2ms (Rust cache) | 75× |

---

## Key Performance Strategies

### 1. Parser — The Biggest Win

**Current**: Python's `robot.api.parsing.get_model()` creates a full in-memory AST using Python objects. For a 1000-line test file, this takes ~80ms. For a 500-file workspace, parsing alone takes ~40 seconds.

**Rust approach**:
- Use `logos` for lexing (generates highly optimized DFA, ~1GB/s throughput)
- Hand-written recursive descent parser: zero allocations for token iteration
- Arena allocator (`bumpalo`) for AST node allocation — entire file's AST in one contiguous block, freed at once
- Parse files in parallel via `rayon` during workspace initialization

```rust
// Arena-allocated AST
let arena = Bump::new();
let ast = parse_file(&source, &arena);  // All nodes in arena
// ... use ast ...
// arena is dropped, entire AST freed in O(1)
```

**Incremental parsing**: Cache parsed AST keyed by (file path, content hash). On document change, only re-parse changed files.

---

### 2. Parallel Workspace Analysis

**Current**: Python asyncio single-threaded event loop. Although `asyncio.gather` is used, CPU-bound analysis tasks block the event loop.

**Rust approach**:
- `tokio` multi-threaded runtime with all available CPU cores
- Analysis tasks are CPU-bound → spawn on `tokio::task::spawn_blocking` or `rayon` thread pool
- Files in a workspace are analyzed in parallel (no sequential dependency unless one file imports another)
- Dependency-aware parallel analysis: build an import DAG, analyze in topological order, parallelize within each level

```
Import DAG for workspace:
  base_keywords.resource ← (no imports)
  common.resource ← base_keywords.resource
  login_tests.robot ← common.resource
  cart_tests.robot ← common.resource
  
Analysis order:
  Level 0 (parallel): base_keywords.resource
  Level 1 (parallel): common.resource
  Level 2 (parallel): login_tests.robot, cart_tests.robot
```

---

### 3. Zero-Copy Text Document

**Current**: Python strings are immutable; incremental edits create new string copies. A 10KB file with a single character edit creates a new 10KB string.

**Rust approach**:
- `ropey` rope data structure: O(log n) character insert/delete
- UTF-16 offset conversion (required by LSP) is O(log n) with ropey
- Semantic tokens and diagnostics store byte offsets internally; convert to LSP UTF-16 only at serialization time

---

### 4. Lazy Library Loading

**Current**: On workspace open, RobotCode eagerly loads all libraries referenced in any file. For a large project with 20+ libraries, this can take 30 seconds.

**Rust approach**:
- Libraries are loaded lazily: only when a file that imports them is opened/analyzed
- Background loading queue: when a file is opened, enqueue its imports for loading; provide partial completions while loading
- Loading progress reported via LSP `$/progress` notification
- Persistent disk cache eliminates re-loading on VS Code restart

---

### 5. Efficient Completion

**Current**: Completion scans all keywords in all imported libraries linearly, then filters by prefix. For a workspace with 2000 keywords, this is O(n) on every keystroke.

**Rust approach**:
- Build an in-memory index: `HashMap<NormalizedName, Vec<KeywordRef>>` at workspace load time
- Prefix search using a radix tree (`radix-trie` or `fst` crate)
- Completion is O(log n) for prefix lookup
- BDD prefix stripping (`Given`/`When`/`Then`/`And`) handled before index lookup

```rust
struct KeywordIndex {
    // Normalized name → keyword locations
    by_name: HashMap<SmolStr, Vec<KeywordId>>,
    // For prefix completion
    prefix_tree: fst::Map<Vec<u8>>,
}
```

---

### 6. Diagnostic Caching with Fine-Grained Invalidation

**Current**: Any change to a resource file triggers re-analysis of all files that import it, recursively. A change to a base resource file can trigger re-analysis of hundreds of files.

**Rust approach**:
- Track which **keywords/variables** actually changed (not just which file changed)
- If a keyword's signature didn't change, files using it don't need re-analysis
- Implement a "dirty bit" system: mark files dirty only if their dependencies' public API changed

```rust
struct AnalysisCache {
    // Per-file analysis result
    results: DashMap<FileId, Arc<FileAnalysis>>,
    // Import dependency graph (reverse edges for invalidation)
    dependents: DashMap<FileId, HashSet<FileId>>,
    // Per-file exported symbols (to detect API changes)
    exports: DashMap<FileId, Arc<ExportedSymbols>>,
}
```

---

### 7. Semantic Token Streaming

**Current**: Semantic tokens for a whole file are computed as a Python list, then serialized to JSON. For a 1000-line file, this means building a list of thousands of token deltas.

**Rust approach**:
- Compute semantic tokens in a single AST traversal pass
- Write token deltas directly to a `Vec<u32>` (LSP format) without intermediate representation
- Use SIMD (via `std::simd` or `packed_simd`) for delta encoding of token positions

---

### 8. JSON Serialization

**Current**: The LSP type model in `core/lsp/types.py` (7,407 lines!) uses a custom dataclasses serializer with runtime type introspection. Every LSP message serialization does reflection.

**Rust approach**:
- `serde_json` with `#[derive(Serialize, Deserialize)]` — zero-overhead compile-time serialization
- `lsp-types` crate provides all LSP types pre-annotated with serde derives
- No runtime type inspection — pure compiled codegen

---

### 9. File Watching

**Current**: Python `watchdog` uses OS-specific backends but has ~100–500ms event latency on Linux (inotify) and up to 1 second on macOS (FSEvents polling fallback).

**Rust approach**:
- `notify` crate with `RecommendedWatcher` — uses kqueue/inotify/FSEvents natively
- Event debouncing: collect events for 50ms, then process as a batch
- Ignore transient writes (editors write to temp file then rename)

---

### 10. Memory Efficiency

**Current**: Python objects have ~50 bytes overhead each. An AST with 10,000 nodes costs ~500KB just in object overhead.

**Rust approach**:
- Enum-based AST nodes: 20–32 bytes each depending on variant
- `SmolStr` for identifier strings (inline storage for strings ≤ 22 bytes — most RF identifiers qualify)
- `Arc<str>` for shared strings (library names, keyword names appearing many times)
- Interned strings for high-frequency tokens (section headers, built-in keyword names)

---

## Benchmark Plan

### Micro-benchmarks (Criterion)
Run on every PR to catch regressions:

```
benches/
  parse_small.rs     # Parse a 100-line .robot file
  parse_large.rs     # Parse a 2000-line .robot file
  semantic_tokens.rs # Generate semantic tokens for benchmark files
  completion.rs      # Completion request with 2000-keyword workspace
  hover.rs           # Hover request latency
  json_serialize.rs  # LSP message serialization
```

### Integration Benchmarks (real workspaces)
Run on representative test suites:

| Benchmark Corpus | Files | Keywords | Python Baseline | Rust Target |
|---|---|---|---|---|
| Small project | 20 | 200 | ~2s startup | <100ms |
| Medium project | 100 | 1000 | ~8s startup | <300ms |
| Large project | 500 | 5000 | ~30s startup | <1s |
| XL project (customer) | 2000 | 20000 | ~120s startup | <5s |

### Latency Benchmarks (LSP request/response)
Measured via VS Code extension timing logs:

| Request | Current P50 | Current P99 | Rust Target P50 | Rust Target P99 |
|---|---|---|---|---|
| `textDocument/completion` | 150ms | 800ms | 5ms | 30ms |
| `textDocument/hover` | 50ms | 300ms | 2ms | 15ms |
| `textDocument/definition` | 80ms | 500ms | 3ms | 20ms |
| `textDocument/references` | 3000ms | 10000ms | 100ms | 500ms |
| `textDocument/semanticTokens/full` | 15ms | 100ms | 0.5ms | 5ms |
| Diagnostic publish delay (after save) | 500ms | 2000ms | 20ms | 100ms |

---

## Profiling Strategy

During development, profile with:
- `cargo flamegraph` — CPU flame graphs
- `heaptrack` / `valgrind --tool=massif` — Memory profiling
- `tokio-console` — Async task visualization
- VS Code timing logs (LSP `$/logTrace` messages)

Key hotspots to watch:
1. Parser token allocation
2. `HashMap` lookups in keyword resolution
3. `Arc` clone overhead in cache
4. JSON serialization for large completion lists
5. File system I/O during import resolution
