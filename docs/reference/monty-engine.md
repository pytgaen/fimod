# Monty Engine — Capabilities & Security Model

Monty is a Python interpreter written in Rust from scratch by Pydantic. It is **not** CPython with restrictions, nor Python compiled to WASM. It is a custom bytecode VM that uses Ruff's parser to convert Python source into its own bytecode format.

Fimod uses Monty (v0.0.8) as its execution engine for mold scripts.

**Source**: [pydantic/monty](https://github.com/pydantic/monty) — [blog post](https://pydantic.dev/articles/pydantic-monty)

## Python Language Support

### Supported

| Feature | Notes |
|---------|-------|
| Functions | sync and async, closures, default args, `*args`/`**kwargs` |
| f-strings | Full support: `f'{x}'`, `f'{x:.2f}'`, `f'{x!r}'`, `f'{x=}'` (debug), nested specs |
| Comprehensions | list, dict, set, generator expressions |
| Type hints | Annotations preserved, used for type checking |
| Dataclasses | When defined on the host side |
| Async/await | `async def`, `await`, `asyncio.gather` |
| Exceptions | `try`/`except`/`finally`/`raise`, tracebacks |
| Walrus operator | `:=` assignment expressions |
| Chain comparisons | `a < b < c` |
| Lambda | `lambda x: x + 1` |
| Unpacking | `a, *b = [1, 2, 3]`; PEP 448 generalised: `{**a, **b}`, `[*a, *b]` |
| Tuple comparison | `(1, 2) < (1, 3)`, `>=`, `<=` |
| Named tuples | `namedtuple` support |
| Assert | `assert expr` with optional message |
| Frozensets | Immutable set type |
| Bytes | `b"..."` byte strings, comparison operators |
| Long integers | Arbitrary precision |
| Augmented subscript | `data["count"] += 1`, `items[0] *= 2` |
| Set/frozenset operators | `s1 \| s2`, `s1 & s2`, `s1 - s2`, `s1 ^ s2`; dict view operators |
| `str` comparison | `"a" < "b"`, `>=`, `<=` |

### Not Yet Supported

| Feature | Status |
|---------|--------|
| Classes | Coming soon |
| Match statements | Coming soon |
| Context managers (`with`) | Coming soon |
| Dict merge operator | `a \| b` not supported — use `{**a, **b}` or `a.update(b)` |
| Third-party packages | Will probably never be supported |
| Full standard library | Only selected modules |

### Built-in Functions

Standard Python builtins: `len`, `range`, `enumerate`, `zip`, `map`, `filter`, `sorted`, `reversed`, `sum`, `min`, `max`, `abs`, `round`, `isinstance`, `getattr`, `type`, `id`, `repr`, `str`, `int`, `float`, `bool`, `list`, `dict`, `set`, `tuple`, `print`, `hash`, etc.

**Not available**: `open`, `exec`, `eval`, `compile`, `__import__`, `input`.

### Standard Library Modules

| Module | Status |
|--------|--------|
| `sys` | Partial (version info) |
| `typing` | Supported (TYPE_CHECKING, annotations) |
| `asyncio` | Supported (gather, run) |
| `pathlib` | Supported (via OsAccess — see Security section) |
| `os` | Partial (getenv only — see Security section) |
| `re` | Supported — compile, search, match, fullmatch, findall, sub, split, finditer, escape; flags: IGNORECASE, MULTILINE, DOTALL, ASCII |
| `math` | Supported — ~50 functions (floor, ceil, sqrt, log, sin, cos, factorial, gcd, lcm, comb…) + constants (pi, e, tau, inf, nan) |
| `datetime` | Planned |
| `json` | Planned |

## External Function Mechanism

Monty provides a controlled bridge between sandbox code and host capabilities through **external functions**. This is the primary extension mechanism.

Since v0.0.8, external functions are resolved **dynamically at runtime** via a `NameLookup` suspension. When the VM first encounters an unknown name, it yields to the host to resolve it. The host returns a `Function` object if the name is a known external function, or `Undefined` to trigger a `NameError`. The resolved value is then cached in the namespace for subsequent calls.

```
Sandbox code calls re_sub("a", "b", text)
    ↓
Monty yields RunProgress::NameLookup { name: "re_sub" }   ← first access only
    ↓
Host: name in known list → resume(NameLookupResult::Value(MontyObject::Function))
    ↓
Monty yields RunProgress::FunctionCall(FunctionCall { function_name: "re_sub", args: [...] })
    ↓
Host (fimod) dispatches to Rust regex implementation
    ↓
Host returns result → call.resume(result, print) → Monty resumes
```

In fimod, external functions provide: regex (`re_*`), dot-path access (`dp_*`), iterators (`it_*`), hashing (`hs_*`), exit control (`set_exit`), and format control (`set_format`, `set_output_file`).

**Note on `re_*` vs `import re`**: since v0.0.8, both are available. `re_*` returns a plain dict `{"match", "start", "end", "groups", "named"}` — convenient for data transformation — and has configurable ReDoS protection (`FIMOD_REGEX_BACKTRACK_LIMIT`). `import re` supports flags (`re.IGNORECASE`, etc.), `fullmatch`, `compile`, `finditer`, `escape`, `maxsplit`, and catchable `re.error` exceptions. Both use the same fancy-regex engine.

## Security Model — Inverted Sandbox

### Design Philosophy

Traditional sandboxes start with full access and try to restrict. Monty inverts this:

> **Start from nothing, then selectively grant capabilities.**

By default, Monty code has:

- No filesystem access
- No network access
- No environment variable access
- No process spawning
- No `open()`, `exec()`, `eval()`
- Strict resource limits (memory, recursion, execution time)

### The OsAccess Mechanism

Since Monty v0.0.7 (PR [#85](https://github.com/pydantic/monty/pull/85)), Monty supports a **sandboxed filesystem** through `pathlib` and `os` modules. Here's how it works:

1. Sandbox code uses standard Python: `Path("/data/file.csv").read_text()`
2. Monty yields a `RunProgress::OsCall` to the host
3. The **host decides** what to do:
   - **Grant access**: Implement `OsAccess` trait to serve files (e.g., from a virtual filesystem)
   - **Deny access**: Return `None` (what fimod does)

This means `pathlib` and `os.getenv()` are **syntactically valid** in mold scripts, but their behavior is entirely controlled by the host application.

### How Fimod Handles OsAccess

Fimod **does not implement filesystem access**. All `OsCall` requests return `None`:

```rust
// engine.rs
RunProgress::OsCall(call) => {
    progress = call
        .resume(MontyObject::None, print)  // always returns None
        .map_err(|e| anyhow::anyhow!("Python error in mold:\n{e}"))?;
}
```

**Verified behavior in fimod** (covered by integration tests in `tests/cli/sandbox.rs`):

| Operation | Result | Test |
|-----------|--------|------|
| `Path("/etc/passwd").exists()` | `null` | `test_sandbox_pathlib_exists_returns_null` |
| `Path("/etc/passwd").read_text()` | `"None"` | `test_sandbox_pathlib_read_text_returns_null` |
| `os.getenv("HOME")` | `null` | `test_sandbox_os_getenv_returns_null` |
| `os.getenv("PATH")` | `null` | `test_sandbox_os_getenv_returns_null` |
| `open("/etc/passwd")` | `NameError` | `test_sandbox_open_not_defined` |
| `import subprocess` | Fails | `test_sandbox_no_subprocess` |
| `import socket` | Fails | `test_sandbox_no_socket` |

These tests serve as a **regression guard**: if Monty's behavior changes or fimod's OsCall handling is modified, these tests will catch it.

### Resource Limits

Monty supports configurable limits through the `LimitTracker` trait:
- **Memory**: Cap total allocation
- **Recursion depth**: Prevent stack overflow
- **Execution time/steps**: Prevent infinite loops

Fimod currently uses `NoLimitTracker` (no limits enforced).

## Performance

| Metric | Value |
|--------|-------|
| Startup latency | ~0.004ms |
| Package size | ~4.5MB |
| Memory overhead | ~5MB |
| Snapshot size | Single-digit KB |

For comparison: Docker startup is ~195ms, Pyodide ~2800ms.

## What This Means for Fimod Mold Authors

1. **You can use f-strings** — `f"Hello {name}"` works perfectly
2. **You can use comprehensions** — `[x * 2 for x in data]` works
3. **You can use closures and lambdas** — functional patterns work
4. **You can use `import re`** — native regex module available; `re.search`, `re.sub`, `re.findall`, etc.
5. **You can use `import math`** — `math.floor`, `math.sqrt`, `math.factorial`, `math.pi`, etc.
6. **You can merge dicts with `{**a, **b}`** — PEP 448 unpacking is supported; `a | b` is not
7. **You cannot read files** — `Path(...)` calls return `None` in fimod
8. **You cannot access env vars via os** — `os.getenv(...)` returns `None`; use the `env` parameter with `--env PATTERN` instead
9. **You cannot import pip packages** — no `requests`, `pandas`, etc.
10. **You cannot define classes** — use dicts and functions instead
11. **All I/O goes through fimod** — data in via `data` parameter, extra context via `args`, `env`, `headers`, data out via `return`
12. **`re_*` vs `import re`** — use `re_*` when you want a structured dict result or ReDoS protection; use `import re` when you need flags, `fullmatch`, `compile`, `finditer`, `escape`, or catchable `re.error`

## Interactive REPL

The `fimod monty repl` command opens an interactive Python session powered by Monty. Use it to experiment, prototype mold logic, and explore what Monty supports before writing a full transform.

```
$ fimod monty repl
Monty REPL v0.0.8 — fimod v0.1.0-alpha.1 (exit or Ctrl+D to quit)
>>> data = {"name": "Alice", "age": 30}
>>> data["name"].upper()
'ALICE'
>>> import math
>>> math.sqrt(data["age"])
5.477225575051661
>>> import re
>>> re.sub(r"\d+", "XX", "born in 1994")
'born in XX'
>>> exit
```

Multi-line input (functions, loops, if blocks) is handled automatically: the REPL detects incomplete syntax and waits for continuation lines.
