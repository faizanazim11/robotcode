#!/usr/bin/env python3
"""RobotCode Python Bridge — JSON-over-stdio request/response loop.

This script is spawned by the Rust ``SubprocessBridge`` as a long-lived
subprocess.  It reads newline-delimited JSON requests from stdin and writes
newline-delimited JSON responses to stdout.  All diagnostic/log output is
written to stderr so that the Rust side can surface it in the Output panel.

Protocol (NDJSON):
  stdin  → {"id": 1, "method": "rf_version", "params": {}}
  stdout ← {"id": 1, "result": {"version": "7.1.1", "major": 7, "minor": 1, "patch": 1}}
  stdout ← {"id": 2, "error": {"code": -32000, "message": "<traceback>"}}
"""

from __future__ import annotations

import json
import sys
import traceback
from typing import Any


# ---------------------------------------------------------------------------
# Method implementations
# ---------------------------------------------------------------------------

def _rf_version(_params: dict) -> dict:
    """Return the installed Robot Framework version."""
    import robot.version as _v  # type: ignore[import]

    ver: str = _v.VERSION
    parts = ver.split(".")
    major = int(parts[0]) if len(parts) > 0 else 0
    minor = int(parts[1]) if len(parts) > 1 else 0
    patch = int(parts[2]) if len(parts) > 2 else 0
    return {"version": ver, "major": major, "minor": minor, "patch": patch}


def _normalize(params: dict) -> dict:
    """RF-style normalized comparison string."""
    from robot.utils import normalize  # type: ignore[import]

    value: str = params["value"]
    remove_underscores: bool = params.get("remove_underscores", True)
    result = normalize(value, ignore=("_",) if remove_underscores else ())
    return {"normalized": result}


def _embedded_args(params: dict) -> dict:
    """Parse embedded argument pattern from a keyword name."""
    try:
        from robot.running.arguments.embedded import EmbeddedArguments  # type: ignore[import]
    except ImportError:
        return {"name": params["pattern"], "args": [], "regex": ""}

    name: str = params["pattern"]
    try:
        embedded = EmbeddedArguments.from_name(name)
    except Exception:
        return {"name": name, "args": [], "regex": ""}

    if embedded is None:
        return {"name": name, "args": [], "regex": ""}

    # RF 7.x: .args is a tuple/list of plain strings; .name is the compiled regex.
    # RF < 6.x: .args may be objects with a .name attribute.
    raw_args = embedded.args if embedded.args else []
    if raw_args and hasattr(raw_args[0], "name"):
        args = [a.name for a in raw_args]
    else:
        args = [str(a) for a in raw_args]

    # .name in RF 7+ is the compiled regex object; try to get its pattern.
    regex_obj = getattr(embedded, "name", None)
    if hasattr(regex_obj, "pattern"):
        regex = regex_obj.pattern
    elif hasattr(embedded, "pattern"):
        p = embedded.pattern
        regex = p.pattern if hasattr(p, "pattern") else str(p)
    else:
        regex = ""

    return {"name": name, "args": args, "regex": regex}


def _variables_doc(params: dict) -> dict:
    """Load a Robot Framework variables file and return its variables."""
    import os

    path: str = params["path"]
    args: list = params.get("args", [])
    base_dir: str = params.get("base_dir", os.path.dirname(path))

    try:
        from robot.variables.filesetter import VariableFileSetter  # type: ignore[import]
        from robot.variables.store import VariableStore  # type: ignore[import]

        store = VariableStore(None)
        setter = VariableFileSetter(store)
        setter.set(path, args)
        variables = []
        for name, value in store.as_dict().items():
            variables.append(
                {
                    "name": name,
                    "value": _safe_repr(value),
                    "source": path,
                    "lineno": 0,
                }
            )
        return {"variables": variables}
    except Exception:
        # Fallback: try older RF API
        try:
            from robot.variables import Variables  # type: ignore[import]

            var_obj = Variables()
            var_obj.set_from_file(path, args)
            variables = []
            for name, value in var_obj.as_dict().items():
                variables.append(
                    {
                        "name": str(name),
                        "value": _safe_repr(value),
                        "source": path,
                        "lineno": 0,
                    }
                )
            return {"variables": variables}
        except Exception as inner:
            raise RuntimeError(f"Failed to load variables file {path!r}: {inner}") from inner


def _safe_repr(value: Any) -> str:
    try:
        return json.dumps(value)
    except (TypeError, ValueError):
        return repr(value)


def _library_doc(params: dict) -> dict:
    """Introspect a Robot Framework keyword library via LibraryDocumentation."""
    import os

    name: str = params["name"]
    args: list = params.get("args", [])
    base_dir: str = params.get("base_dir", os.getcwd())
    python_path: list = params.get("python_path", [])
    variables: dict = params.get("variables", {})

    # Extend sys.path temporarily
    import sys as _sys

    original_path = _sys.path[:]
    try:
        for p in python_path:
            if p not in _sys.path:
                _sys.path.insert(0, p)

        from robot.libdocpkg import LibraryDocumentation  # type: ignore[import]

        doc = LibraryDocumentation(
            name,
            doc_format="ROBOT",
        )

        keywords = []
        for kw in doc.keywords:
            kw_args = []
            try:
                for arg in kw.args:
                    kw_args.append(_serialize_arg(arg))
            except Exception:
                pass

            keywords.append(
                {
                    "name": kw.name,
                    "args": kw_args,
                    "doc": kw.doc or "",
                    "tags": list(kw.tags) if kw.tags else [],
                    "source": getattr(kw, "source", None),
                    "lineno": getattr(kw, "lineno", None),
                }
            )

        inits = []
        for init in (doc.inits or []):
            init_args = []
            try:
                for arg in init.args:
                    init_args.append(_serialize_arg(arg))
            except Exception:
                pass
            inits.append({"args": init_args, "doc": init.doc or ""})

        return {
            "name": doc.name,
            "doc": doc.doc or "",
            "version": doc.version or "",
            "scope": str(doc.scope) if doc.scope else "GLOBAL",
            "named_args": bool(getattr(doc, "named_args", True)),
            "keywords": keywords,
            "inits": inits,
            "typedocs": [],
        }
    finally:
        _sys.path = original_path


def _serialize_arg(arg: Any) -> dict:
    """Convert an argument spec object to a JSON-serializable dict."""
    name = getattr(arg, "name", str(arg))
    kind_raw = getattr(arg, "kind", None)
    kind = str(kind_raw) if kind_raw is not None else "POSITIONAL_OR_NAMED"
    default_val = _arg_default(arg)
    types_raw = getattr(arg, "types", None) or []
    types = [str(t) for t in types_raw]
    return {"name": name, "kind": kind, "default": default_val, "types": types}


def _arg_default(arg: Any) -> Any:
    """Return the default value string for an argument, or None if there is none."""
    default_repr = getattr(arg, "default_repr", None)
    if default_repr is not None:
        return default_repr
    default_raw = getattr(arg, "default", None)
    if default_raw is not None:
        return _safe_repr(default_raw)
    return None


def _discover(params: dict) -> dict:
    """Discover tests using Robot Framework's TestSuiteBuilder."""
    import os

    paths: list = params.get("paths", [])
    include_tags: list = params.get("include_tags", [])
    exclude_tags: list = params.get("exclude_tags", [])
    python_path: list = params.get("python_path", [])

    import sys as _sys

    original_path = _sys.path[:]
    try:
        for p in python_path:
            if p not in _sys.path:
                _sys.path.insert(0, p)

        from robot.running.builder import TestSuiteBuilder  # type: ignore[import]

        builder = TestSuiteBuilder()
        suites_out = []

        for path in paths:
            try:
                suite = builder.build(path)
                suites_out.append(_serialize_suite(suite))
            except Exception as exc:
                print(f"[bridge] discover warning for {path!r}: {exc}", file=sys.stderr)

        return {"suites": suites_out}
    finally:
        _sys.path = original_path


def _serialize_suite(suite: Any) -> dict:
    tests = []
    for test in getattr(suite, "tests", []):
        tests.append(
            {
                "name": test.name,
                "tags": list(test.tags) if test.tags else [],
                "lineno": getattr(test, "lineno", None),
            }
        )

    children = []
    for child in getattr(suite, "suites", []):
        children.append(_serialize_suite(child))

    return {
        "name": suite.name,
        "source": str(suite.source) if suite.source else None,
        "tests": tests,
        "suites": children,
    }


# ---------------------------------------------------------------------------
# Dispatch table
# ---------------------------------------------------------------------------

_METHODS = {
    "rf_version": _rf_version,
    "normalize": _normalize,
    "embedded_args": _embedded_args,
    "variables_doc": _variables_doc,
    "library_doc": _library_doc,
    "discover": _discover,
}


# ---------------------------------------------------------------------------
# Main request/response loop
# ---------------------------------------------------------------------------

def _json_default(obj: Any) -> Any:
    """Custom JSON serializer for types not handled by the default encoder."""
    import pathlib

    if isinstance(obj, pathlib.PurePath):
        return str(obj)
    raise TypeError(f"Object of type {type(obj).__name__} is not JSON serializable")


def _send(obj: dict) -> None:
    print(json.dumps(obj, default=_json_default), flush=True)


def main() -> None:
    print("[bridge] RobotCode Python bridge started", file=sys.stderr, flush=True)

    for raw_line in sys.stdin:
        raw_line = raw_line.strip()
        if not raw_line:
            continue

        req_id = None
        try:
            req = json.loads(raw_line)
            req_id = req.get("id")
            method = req["method"]
            params = req.get("params", {})

            handler = _METHODS.get(method)
            if handler is None:
                _send(
                    {
                        "id": req_id,
                        "error": {
                            "code": -32601,
                            "message": f"Method not found: {method!r}",
                        },
                    }
                )
                continue

            result = handler(params)
            _send({"id": req_id, "result": result})

        except Exception:
            tb = traceback.format_exc()
            print(f"[bridge] error: {tb}", file=sys.stderr, flush=True)
            _send(
                {
                    "id": req_id,
                    "error": {"code": -32000, "message": tb},
                }
            )

    print("[bridge] stdin closed, exiting", file=sys.stderr, flush=True)


if __name__ == "__main__":
    main()
