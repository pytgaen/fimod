# Monty Engine — Capabilities & Security Model

Monty is a Python interpreter written in Rust from scratch by Pydantic. It is **not** CPython with restrictions, nor Python compiled to WASM. It is a custom bytecode VM that uses Ruff's parser to convert Python source into its own bytecode format.

Fimod uses Monty (v1.0.0) as its execution engine for mold scripts.

**Source**: [pydantic/monty](https://github.com/pydantic/monty) — [blog post](https://pydantic.dev/articles/pydantic-monty)

## Python Language Support

### Supported

| Feature | Notes |
|---------|-------|
| Functions | sync and async, closures, default args, `*args`/`**kwargs` |
| String formatting | `str.format`, `format(value, spec)`, and `%` formatting for strings/bytes |
| Dict merge | `a | b`, `{**a, **b}`, and `a.update(b)` |
| Dynamic execution | `eval`, `exec`, and `locals`, within Monty’s runtime and sandbox |
| f-strings | Full support: `f'{x}'`, `f'{x:.2f}'`, `f'{x!r}'`, `f'{x=}'` (debug), nested specs |
| Comprehensions | list, dict, set, generator expressions |
| Type hints | Annotations preserved, used for type checking |
| Classes and decorators | Classes with `__init__` and methods; function decorators |
| Dataclasses | Defined in molds; `eq` and `frozen` options supported |
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
| Nested subscript assignment | `data[0][1] = x`, `matrix[i][j] = v` |
| Chain assignment | `a = b = 1` assigns the same value to multiple targets |
| Named keyword args | `str.split(sep=",")`, `max(items, key=...)` |
| Set/frozenset operators | `s1 | s2`, `s1 & s2`, `s1 - s2`, `s1 ^ s2`; dict view operators |
| `str` comparison | `"a" < "b"`, `>=`, `<=` |
| Context managers (`with`) | Supported, including `with open(...) as f:` when the host permits `open()` |

Monty's runtime integers are arbitrary precision. At the JSON boundary, fimod
preserves the complete native JSON integer range from `i64::MIN` through
`u64::MAX` as numeric values. Results outside that bridge range are serialized
as decimal strings instead of being rounded through `f64`.

### Not Yet Supported

| Feature | Status |
|---------|--------|
| Match statements | Coming soon |
| Third-party packages | Will probably never be supported |
| Full standard library | Only selected modules |

### Built-in Functions

Standard Python builtins: `len`, `range`, `enumerate`, `zip`, `map`, `filter`, `sorted`, `reversed`, `sum`, `min`, `max` (with `key=` and `default=`), `abs`, `round`, `isinstance`, `getattr`, `hasattr`, `setattr`, `type`, `id`, `repr`, `str`, `int`, `float`, `bool`, `list`, `dict`, `set`, `tuple`, `print`, `hash`, etc.

`open()` is syntactically available, but fimod denies it through the sandbox until filesystem mounts exist.

**Not available**: `compile`, `__import__`, `input`.

### Standard Library Modules

| Module | Status |
|--------|--------|
| `sys` | Version/constants and `print(..., file=sys.stderr)` |
| `typing` | Supported (TYPE_CHECKING, annotations) |
| `asyncio` | Supported (gather, run) |
| `pathlib` | Supported (via `OsFunctionCall` — see Security section) |
| `os` | Partial (getenv only — see Security section) |
| `re` | Supported — compile, search, match, fullmatch, findall, sub, split, finditer, escape; flags: IGNORECASE, MULTILINE, DOTALL, ASCII |
| `math` | Supported — ~50 functions (floor, ceil, sqrt, log, sin, cos, factorial, gcd, lcm, comb…) + constants (pi, e, tau, inf, nan) |
| `datetime` | Supported — `date`, `datetime`, `time`, `timedelta`, `timezone`; arithmetic, `.isoformat()`, `.strftime()`, `.today()`, `.now()`, `.utcnow()` |
| `json` | Supported — `json.dumps()`, `json.loads()`. ~2x faster than CPython for loads, ~1.65x for dumps (v0.0.11 string cache + lookup-table escaping). Rarely needed: fimod handles JSON parsing/serialization in Rust. Useful only for edge cases like building a JSON string inside a text template |
| `collections` | `Counter`, `defaultdict`, `deque`, `namedtuple` |
| `dataclasses` | `dataclass`, `is_dataclass`; `eq` and `frozen` options |
| `functools` | `reduce`, `partial` |
| `base64`, `binascii` | Binary/text encodings; decode byte results to strings before returning structured output |
| `itertools` | Includes `takewhile`, `dropwhile`, `filterfalse`, `starmap`, `accumulate`, `batched`, `zip_longest` |
| `unicodedata` | Unicode character properties and normalization |
| `copy` | `copy`, `deepcopy` |
| `random` | Explicitly seeded generators; unseeded draws requesting host entropy are denied |
| `time` | Clock reads require `allow_clock = true`; sleep is denied |
| `zip(..., strict=True)` | Supported since v0.0.12 — raises `ValueError` on length mismatch |

## External Function Mechanism

Monty provides a controlled bridge between sandbox code and host capabilities through **external functions**. This is the primary extension mechanism.

Since v0.0.8, external functions are resolved **dynamically at runtime** via a `NameLookup` suspension. When the VM first encounters an unknown name, it yields to the host to resolve it. The host returns a `Function` object if the name is a known external function, or `Undefined` to trigger a `NameError`. The resolved value is then cached in the namespace for subsequent calls.

```
With FIMOD_LEGACY_BUILTINS=1, sandbox code calls re_sub("a", "b", text)
    ↓
Monty yields RunProgress::NameLookup { name: "re_sub" }   ← first access only
    ↓
Host: name in known list → resume(NameLookupResult::Value(MontyObject::function(name, None)))
    ↓
Monty yields RunProgress::FunctionCall(FunctionCall { function_name: "re_sub", args: [...] })
    ↓
Host (fimod) dispatches to Rust regex implementation
    ↓
Host returns result → call.resume(result, print) → Monty resumes
```

In fimod, external functions provide dot-path access (`dp_*`), iteration (`it_*`), hashing (`hs_*`), exit control (`set_exit`), format control (`set_input_format`, `set_output_format`, `set_output_file`), and opt-in legacy regex (`re_*`).

**Legacy built-ins**: `re_*` (including `_fancy`), `it_unique`, `it_unique_by`, and `it_flatten` are blocked at host dispatch unless the process has `FIMOD_LEGACY_BUILTINS=1`. Activation emits no warning. New regex molds use `import re`; legacy regex retains its dict results, replacement syntax, and `FIMOD_REGEX_BACKTRACK_LIMIT`. See the [migration guide](built-ins.md#legacy-built-ins).

## Security Model — Inverted Sandbox

### Design Philosophy

Traditional sandboxes start with full access and try to restrict. Monty inverts this:

> **Start from nothing, then selectively grant capabilities.**

By default, Monty code has:

- No network access
- No environment variable access
- No process spawning
- No direct filesystem access. `open()` and `Path.*` route through the host and are denied by fimod unless explicitly implemented.
- `eval()` and `exec()` stay inside Monty; they do not invoke system Python
- Strict resource limits (memory, recursion, execution time)

### The OsFunctionCall Mechanism

Monty exposes host-sensitive operations through typed `OsFunctionCall` suspensions. Here's how it works:

1. Sandbox code uses standard Python: `Path("/data/file.csv").read_text()`
2. Monty yields `RunProgress::OsCall` with a typed operation such as `ReadText`, `Open`, `Getenv`, or `DateTimeNow`
3. The **host decides** what to do:
   - **Grant access**: return the requested value
   - **Deny access**: return `None` for legacy `Path.*` calls or a `PermissionError` for calls that need an object such as `open()`

This means `pathlib`, `open()`, `os.getenv()`, and clock calls are **syntactically valid** in mold scripts, but their behavior is entirely controlled by the host application.

### How Fimod Handles OsFunctionCall

Fimod implements clock and selected environment access through `sandbox.toml`, but **does not implement filesystem access** yet:

```rust
// engine.rs
RunProgress::OsCall(call) => {
    progress = call.resume_with(print, |function_call| {
        sandbox_os_call_result(function_call, policy)
    })?;
}
```

**Verified behavior in fimod** (covered by integration tests in `tests/cli/sandbox.rs`):

| Operation | Result | Test |
|-----------|--------|------|
| `Path("/etc/passwd").exists()` | `null` | `test_sandbox_pathlib_exists_returns_null` |
| `Path("/etc/passwd").read_text()` | `"None"` | `test_sandbox_pathlib_read_text_returns_null` |
| `os.getenv("HOME")` | `null` | `test_sandbox_os_getenv_returns_null` |
| `os.getenv("PATH")` | `null` | `test_sandbox_os_getenv_returns_null` |
| `open("/etc/passwd")` | `PermissionError` | `test_sandbox_open_is_denied` |
| `import subprocess` | Fails | `test_sandbox_no_subprocess` |
| `import socket` | Fails | `test_sandbox_no_socket` |

These tests serve as a **regression guard**: if Monty's behavior changes or fimod's OsCall handling is modified, these tests will catch it.

### Clock, entropy and sleep policy

Fimod explicitly configures Monty's `OsPolicy` to send clock, initial random
entropy and sleep requests to the host. Monty's system defaults therefore do
not bypass fimod's sandbox policy, in either molds or the REPL.

- `allow_clock = true` permits `datetime` clock calls and `time.time()` / related
  clock reads. Explicit fixed-offset timezones passed to `datetime.now(tz)` are
  respected.
- `os.urandom()` and unseeded random draws are denied. Use `random.seed(42)` or
  `random.Random(42)` for reproducible transforms.
- `time.sleep()` and `asyncio.sleep()` are denied, including zero-duration sleeps.
  Clock permission does not grant sleep or entropy access.
- `print(..., file=sys.stderr)` writes to stderr. In debug mode, both Python
  print streams go to stderr.

### Resource Limits

Monty supports configurable limits through its `ResourceTracker`:
- **Memory**: Cap total allocation
- **Recursion depth**: Prevent stack overflow
- **Execution time**: A per-feed budget bounds a mold or REPL snippet across resumptions; host suspension time is excluded by Monty. Fimod also retains its chain deadline accounting.

Fimod uses `ResourceTracker` with hard defaults (`max_duration = 10m`, `max_memory = 2GB`). These defaults apply even without a `sandbox.toml`. See the [Sandbox](../guides/cli-reference.md#sandbox-policy) section for configuring limits via `~/.config/fimod/sandbox.toml` or `--sandbox-file`.

Fimod also supports `max_suspensions`, a host-enforced quota disabled by default
(`0`). It counts every `FunctionCall`, `OsCall`, `NameLookup`, or `ResolveFutures`
suspension before handling it, separately for each mold and each REPL snippet.
Exactly N suspensions are allowed with a quota of N; completion itself costs none.
Exceeding it stops the mold with exit 137 and cannot be caught by Python. The REPL
reports the error and remains usable, with a fresh quota for the next snippet.

`datetime.time` results serialize as ISO time strings, including microseconds and
UTC offsets when present, through both fimod serialization paths.

Since Monty 0.0.20, `max_memory` is **allocator-backed**: the interpreter reads live-byte counters that only a charging global allocator writes. Fimod installs one in `src/mem_limit.rs` — mimalloc wrapped to feed those counters — so the limit stays enforced without giving up mimalloc's performance. A mold that exceeds it is stopped at the interpreter's next checkpoint and reported as `sandbox exploded: max_memory exceeded`.

## Performance

| Metric | Value |
|--------|-------|
| Startup latency | ~0.004ms |
| Package size | ~4.5MB |
| Memory overhead | ~5MB |
| Snapshot size | Single-digit KB |
| `json.loads()` vs CPython | ~2x faster (string cache, optimized dict insertion) |
| `json.dumps()` vs CPython | ~1.65x faster (lookup-table escaping, in-place sort) |

For comparison: Docker startup is ~195ms, Pyodide ~2800ms.

## What This Means for Fimod Mold Authors

1. **You can use f-strings** — `f"Hello {name}"` works perfectly
2. **You can use comprehensions** — `[x * 2 for x in data]` works
3. **You can use closures and lambdas** — functional patterns work
4. **You can use `import re`** — native regex module available; `re.search`, `re.sub`, `re.findall`, etc.
5. **You can use `import math`** — `math.floor`, `math.sqrt`, `math.factorial`, `math.pi`, etc.
6. **You can use `import datetime`** — `datetime.date`, `datetime.datetime`, `datetime.timedelta`, `datetime.timezone`. Datetime objects returned in the output are automatically serialized as ISO 8601 strings
7. **You can use `import collections`** (Monty 0.0.20+) — `Counter`, `defaultdict`, `deque`, `namedtuple`. The `User*` classes are not implemented
8. **You can use `import itertools`** (Monty 0.0.20+) — `count`, `repeat`, `pairwise`, `compress`, `islice`, `chain`, `cycle`
9. **You can use `import dataclasses`** (Monty 0.0.20+) — `@dataclass` and `is_dataclass` work on classes defined in the mold itself; `__post_init__` is not supported and is rejected rather than silently skipped
10. **You can define classes and use decorators** — plain `class` with `__init__` and methods works, and so do function decorators
11. **You can merge dicts with `a | b` or `{**a, **b}`**
12. **Keep `**_` in mold signatures** — `def transform(data, args, **_):` is the recommended convention; fimod passes `args`, `env`, `headers`, and `pipeline` as keyword arguments, and `**_` absorbs anything the mold does not use
13. **You cannot read files** — `Path(...)` calls return `None` in fimod
14. **You cannot access env vars via os unless sandbox policy allows them** — denied `os.getenv(...)` calls return `None`; the `env` parameter with `--env PATTERN` is still the portable fimod-native path
15. **You cannot import pip packages** — no `requests`, `pandas`, etc.
16. **Use `import x`, not `__import__("x")`** — fimod resolves external functions by name and rejects `__import__`
17. **All I/O goes through fimod** — data in via `data` parameter, extra context via `args`, `env`, `headers`, `pipeline`, data out via `return`
18. **Use `import re` for new molds** — `re_*` is deprecated and requires `FIMOD_LEGACY_BUILTINS=1`. Legacy compatibility retains dict results and the configurable `FIMOD_REGEX_BACKTRACK_LIMIT`; native regex has its own backtracking limit. See the [migration guide](built-ins.md#legacy-built-ins).

## Interactive REPL

The `fimod monty repl` command opens an interactive Python session powered by Monty. Use it to experiment, prototype mold logic, and explore what Monty supports before writing a full transform. The REPL resolves the same sandbox policy as `fimod shape`, including `--sandbox-file <path>` and `--sandbox-file=""`. Mold-only helper families such as `re_*` and `dp_*` are not imported into the REPL.

```
$ fimod monty repl
Monty REPL v1.0.0 — fimod v0.11.0 (exit or Ctrl+D to quit)
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
