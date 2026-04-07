# 06 — Risk Register & Mitigation

## Risk Summary

| ID | Risk | Likelihood | Impact | Mitigation |
|----|------|-----------|--------|-----------|
| R1 | RF parser incompatibility with edge cases | HIGH | HIGH | Extensive corpus testing, reference fuzzing |
| R2 | Python bridge latency unacceptable | MEDIUM | HIGH | Aggressive caching, PyO3 fallback |
| R3 | Robot Framework API breaks with new RF version | HIGH | MEDIUM | Isolate bridge, version-pin, quickrelease process |
| R4 | team Rust expertise insufficient | MEDIUM | HIGH | Training period, phased approach |
| R5 | Diagnostic parity regression | MEDIUM | HIGH | Snapshot test suite, parallel-run validation |
| R6 | VS Code/IntelliJ integration breakage | LOW | HIGH | Feature flag: dual-mode binary selection |
| R7 | Windows cross-compilation complexity | MEDIUM | MEDIUM | Early CI setup, MSVC toolchain |
| R8 | Migration takes longer than estimated | HIGH | MEDIUM | Phases are independently shippable |
| R9 | Community contribution friction (Rust barrier) | MEDIUM | MEDIUM | Keep Python packages usable during transition |
| R10 | Memory safety issues in Python bridge FFI | LOW | HIGH | Use subprocess model by default |

---

## Detailed Risk Analysis

### R1 — RF Parser Incompatibility

**Description**: Robot Framework's `.robot` syntax has many edge cases: context-sensitive tokens, multi-line constructs, variable scoping in argument lists, embedded argument patterns, template syntax, inline `IF`/`WHILE` (RF 5+), `TRY`/`EXCEPT` (RF 5+), `VAR` syntax (RF 7+). The Python parser has accumulated years of bug fixes. A Rust re-implementation will miss edge cases.

**Evidence from codebase**:
- `namespace_analyzer.py` has 2,000 lines of special-case handling
- `model_helper.py` has 790 lines of AST traversal helpers, many handling edge cases
- The test matrix covers RF 5.0, 6.0, 7.0, 7.4 — each version has syntax changes

**Mitigation**:
1. Run the Rust parser against the entire `tests/` corpus on every PR
2. Collect a large corpus of public Robot Framework projects from GitHub (10,000+ files) and use as a fuzzing target
3. Use property-based testing (`proptest`): generate random valid RF files, verify Rust and Python parsers produce equivalent ASTs
4. Implement a `--validate-parser` mode that runs both parsers and diffs output
5. During Phases 3–5, run Rust analysis in parallel with Python and log any differences (shadow mode)
6. Keep the Python bridge's `get_model` available as a fallback for files the Rust parser fails on

**Go/No-Go Criterion**: Zero discrepancies on a 10,000-file public RF corpus before shipping Phase 2.

---

### R2 — Python Bridge Latency

**Description**: Library introspection requires spawning Python, importing the library, and extracting keyword metadata. For large libraries (Selenium: ~300 keywords), this can take 1–3 seconds. If users experience slow completions on first use, it degrades UX.

**Mitigation**:
1. **Persistent disk cache**: Cache `LibraryDoc` results to `~/.cache/robotcode/`. On VS Code startup, serve cached results immediately (no bridge call). Background-refresh stale cache entries.
2. **Progressive completions**: Show completions from already-loaded libraries immediately; add more as bridge returns results. Use LSP `isIncomplete` flag.
3. **Eager library loading**: On workspace open, pre-load all libraries referenced in any file in the background, before the user starts typing.
4. **PyO3 feature**: For users who build from source or when deploying in a known-Python-version environment, use PyO3 for in-process Python — eliminates bridge latency entirely.
5. **Partial introspection**: For completion, only fetch keyword names first (fast); fetch full argument specs lazily when needed for hover/signature help.

**Acceptance Criterion**: Cold-start completion for a new library ≤ 2 seconds; warm-start (disk cache hit) ≤ 50ms.

---

### R3 — Robot Framework API Changes

**Description**: RobotCode is tested against RF 5.0–7.4. New RF versions (8.x in future) may change internal APIs, breaking `helper.py`. Since the Rust codebase uses RF only via the bridge, this is partially isolated — but `helper.py` still needs maintenance.

**Mitigation**:
1. `helper.py` uses **public RF APIs only** (`robot.libdocpkg`, `robot.api`), not internal modules
2. The existing Python code already handles version differences (`RF_VERSION` checks) — port this logic to `helper.py`
3. Pin `helper.py` to use the API subset that has been stable since RF 4.0
4. Maintain a separate test suite for `helper.py` covering RF 5.0–7.4 (same matrix as current)
5. New RF version support: update `helper.py`, re-test, ship quickly (no Rust changes required for most RF updates)

---

### R4 — Team Rust Expertise

**Description**: If the development team is primarily Python engineers, the Rust learning curve (borrow checker, lifetimes, async) adds significant development friction.

**Mitigation**:
1. **Phased approach**: Phase 1 (core data types) is good Rust practice — start there before tackling complex async code
2. **Hire or consult**: Consider engaging a Rust consultant for Phase 2 (parser) and Phase 3 (async LSP)
3. **Leverage tower-lsp**: Using `tower-lsp` abstracts most of the async complexity — the team writes handler functions, not protocol machinery
4. **Incremental learning**: Phases 1–2 can be done by Python engineers learning Rust; Phases 3–6 benefit from Rust experience
5. **Code review**: All Rust PRs must be reviewed by someone with Rust experience (external reviewer if needed)

---

### R5 — Diagnostic Parity Regression

**Description**: The most critical correctness requirement is that the Rust diagnostics engine produces the same diagnostics (same codes, same positions, same messages) as the Python implementation. Any regression is a user-visible quality degradation.

**Mitigation**:
1. **Snapshot test suite**: For every `.robot` test file in `tests/`, snapshot the complete list of diagnostics (code, range, message, severity). Rust must match exactly.
2. **Shadow mode**: During Phase 5 beta, run both Python and Rust analysis in parallel. Log any divergence. Ship only when divergence rate is 0%.
3. **Property-based testing**: Generate random RF files, compare Python and Rust diagnostic lists.
4. **Regression database**: Track every known diagnostic scenario from bug reports as a test case.
5. **Graduated rollout**: Enable Rust diagnostics per-feature (undefined keywords first, then variables, then type checking) so issues are isolated.

---

### R6 — VS Code / IntelliJ Integration Breakage

**Description**: The VS Code extension and IntelliJ plugin consume the language server via LSP. Any difference in initialization sequence, capabilities declaration, or protocol behavior could break the editor integration.

**Mitigation**:
1. **Feature flag**: Add `"robotcode.useRustServer": false` (default `false` during beta) to `package.json`. Users opt in.
2. **Capability parity**: Declare exactly the same `ServerCapabilities` in `initialize` response as the Python server.
3. **Integration test suite**: Use `@vscode/test-electron` to run VS Code with the Rust server and test all major features.
4. **Slow rollout**: Beta → RC → GA over 3 release cycles (~3 months).
5. **Fallback**: If Rust server exits with error, VS Code extension automatically falls back to Python server.

---

### R7 — Windows Cross-Compilation

**Description**: The extension must ship binaries for Windows. Cross-compiling Rust for `x86_64-pc-windows-msvc` from Linux CI is non-trivial, especially with any C FFI dependencies.

**Mitigation**:
1. Set up Windows GitHub Actions runners for native compilation (not cross-compilation)
2. Avoid C FFI in Rust crates where possible (pure-Rust alternatives preferred)
3. If PyO3 is used: compile on Windows natively; don't cross-compile
4. Use `cargo-xwin` or `zig-cc` for Linux→Windows cross-compilation as a fallback
5. Test Windows binary on each PR using Windows GitHub Actions runner

---

### R8 — Timeline Overrun

**Description**: The 18–24 month estimate assumes 2–3 engineers working full-time. Scope creep, underestimated complexity (especially the diagnostics engine), or staffing issues could extend this.

**Mitigation**:
1. **Each phase ships independently**: If Phase 2 (parser) takes longer, Phases 3 and 4 don't block.
2. **MVP approach**: Phase 6 (LSP features) can ship with a subset of features (diagnostics, completion, hover, go-to-definition) for initial beta.
3. **Python remains functional**: Users are never blocked. Only the internal server switches.
4. **Monthly milestone reviews**: Re-estimate remaining effort after each phase.
5. **Scope reduction options**: If timeline slips, defer REPL (Phase 8), DAP (Phase 7), or some LSP features (inline values, linked editing ranges) to post-GA.

---

### R9 — Community Contribution Friction

**Description**: Robot Framework is a Python ecosystem. Community contributors who want to fix bugs or add features need to learn Rust, which is a significant barrier.

**Mitigation**:
1. **Python bridge is Python**: `helper.py` and its tests remain Python. Library introspection bug fixes stay accessible to Python contributors.
2. **Good documentation**: Document the Rust crate architecture thoroughly. Provide contribution guides.
3. **Rust is approachable**: For simple bug fixes (diagnostic messages, keyword name matching), Rust code is readable even without deep expertise.
4. **Keep Python packages for reference**: Don't delete Python packages until GA + 6 months. Contributors can study the Python code as a reference implementation.
5. **Issue labels**: Tag issues as `rust-needed` vs `python-bridge` to direct contributors to appropriate code.

---

### R10 — Memory Safety in Python Bridge FFI

**Description**: If using PyO3 (embedded Python), incorrect handling of Python object lifetimes or GIL interactions can cause use-after-free, data races, or interpreter corruption.

**Mitigation**:
1. **Default to subprocess bridge**: No FFI risk in the default configuration.
2. **PyO3 as opt-in feature**: Only adventurous users who compile from source use PyO3.
3. **PyO3 is memory-safe**: PyO3's `Python::with_gil` API makes it impossible to use Python objects without holding the GIL.
4. **Miri testing**: Run PyO3 code under Miri (Rust's undefined behavior detector) in CI.
5. **Fuzz testing**: Fuzz the Python bridge protocol parser to ensure no panic on malformed Python output.

---

## Go/No-Go Criteria for Each Phase

### Phase 2 (Parser) Go/No-Go
- ✅ Rust parser handles all `.robot` files in `tests/` corpus without panic
- ✅ Snapshot tests: 0 discrepancies vs Python parser on test corpus
- ✅ Performance: ≥10× faster than Python on benchmark files
- ✅ Error recovery: parser produces useful partial AST even on malformed input

### Phase 5 (Diagnostics) Go/No-Go
- ✅ All diagnostic codes match Python on all test fixtures (0 regressions)
- ✅ No false positives: Rust emits ≤ diagnostics compared to Python for equivalent input
- ✅ Performance: workspace analysis ≥20× faster than Python
- ✅ Shadow mode: 0% divergence rate over 2-week community beta

### Phase 6 (LSP Features) Go/No-Go
- ✅ All snapshot tests pass
- ✅ Manual VS Code integration test: all documented features work
- ✅ Performance targets met (see [05-performance.md](05-performance.md))
- ✅ No data loss: rename, code actions produce correct edits

### Phase 8 (GA Cutover) Go/No-Go
- ✅ Beta period ≥ 4 weeks with no P0/P1 bugs reported
- ✅ All platform binaries (Linux/macOS/Windows) pass CI
- ✅ VS Code extension published to Marketplace with Rust binary
- ✅ IntelliJ plugin updated and published
- ✅ Fallback to Python server works when Rust binary missing

---

## Alternatives Considered

### Alternative 1: Performance-optimize the Python implementation

Instead of Rust, keep Python but optimize aggressively:
- Use Cython or mypyc to compile hot paths
- Use multiprocessing for parallelism
- Add more caching layers

**Rejected because**: The fundamental bottleneck is Python's interpreter overhead, not algorithmic complexity. The parser and namespace analyzer are already O(n) — Cython would give 2–3× improvement at best. Rust gives 30–100×. Additionally, the GIL prevents true parallelism for CPU-bound analysis.

### Alternative 2: Use tree-sitter

Build on the existing `tree-sitter-robotframework` community grammar instead of writing a parser from scratch.

**Partially accepted**: tree-sitter is a viable option for Phase 2. Pros: incremental parsing built-in, battle-tested. Cons: C library dependency, grammar may not cover all RF edge cases, less control over error recovery. **Recommendation**: evaluate tree-sitter in Phase 2 alongside the Logos-based approach; choose the one that passes the corpus test with better performance.

### Alternative 3: Node.js / TypeScript language server

Write the language server in TypeScript (same language as the VS Code extension).

**Rejected because**: TypeScript would give ~2–5× performance improvement over Python, not 30–100×. The analysis engine is CPU-bound; JavaScript/TypeScript single-threaded event loop is a similar constraint to Python's GIL. Rust is the right choice for a performance-critical analysis engine.

### Alternative 4: Keep Python, add Rust extension modules via PyO3

Write performance-critical hot paths in Rust as Python extension modules (like `orjson`, `pydantic-core`).

**Partially viable as interim step**: This could be done in Phase 2 (RF parser as a Python extension) and Phase 5 (namespace analysis as a Python extension) without rewriting the full LSP stack. This is a lower-risk approach with incremental wins.

**Trade-off**: Achieves 20–50× improvement on specific hot paths but doesn't achieve the architectural simplicity and full parallelism of a pure Rust binary. Recommended as an **optional intermediate milestone** if the full migration proves too slow: ship `robotcode-rf-parser` as a Python extension module (PyO3) to get parser performance improvements in Phase 2 while the rest of the Rust migration continues.
