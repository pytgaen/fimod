# 💡 Concepts

## 🔄 The pipeline

Every fimod invocation runs through the same pipeline:

```
📥 Read → 🔍 Parse → 🐍 transform(data, **_) → 📤 Serialize → ✍️ Write
```

| Step | What happens |
|------|-------------|
| **📥 Read** | Input from file (`-i`), URL (`-i https://...`), stdin, or multiple files combined with `-s` (multi-file slurp). `--no-input` skips this step (`data = None`). |
| **🔍 Parse** | Rust parses the input via serde into native types. |
| **🔁 Convert** | Parsed data becomes a Python-compatible `MontyObject`. |
| **🐍 Execute** | Monty runs `transform`; `data` is positional and context (`args`, `env`, `headers`, `pipeline`) is passed as keyword arguments. Built-ins (`re_*`, `dp_*`, `it_*`, `hs_*`) are available. |
| **🔁 Convert back** | Return value becomes `serde_json::Value`. |
| **📤 Serialize** | Rust serializes the result to the output format (see below). |
| **✍️ Write** | Output goes to file (`-o`), stdout, or input file (`--in-place`). |

!!! tip "Output format resolution"
    If `--output-format` is not specified, fimod resolves the output format automatically:

    1. **Explicit flag** — `--output-format json` always wins
    2. **Output file extension** — `-o result.yaml` → YAML
    3. **Same as input** — if neither is set, the output format matches the input format

    This means **you never need `--output-format` when the output format is the same as the input**.
    Only specify it when converting between formats without an output file.

!!! info "Parsing and serialization are pure Rust"
    Monty only sees Python dicts and lists — it never touches the filesystem or network.

---

## 🧱 What is a mold?

A **mold** is a Python script that defines a `transform(data, **_)` function. It receives the parsed input and returns the transformed result. Keep `**_` in reusable molds so unused and future keyword arguments are ignored safely.

```python
def transform(data, **_):
    return {"count": len(data)}
```

### Three ways to specify a mold

=== "📝 Inline expression"

    ```bash
    fimod s -i data.json -e '[u for u in data if u["active"]]'
    ```

    Best for one-liners. Auto-wrapped into a `transform(..., **_)` function.
    For multi-statement, write `def transform` explicitly inside `-e`.

=== "📄 Script file"

    ```bash
    fimod s -i data.json -m cleanup.py
    ```

    Reusable, version-controlled transforms. Must define `transform(data, **_)`, adding named context parameters before `**_` when needed.

=== "🌐 URL"

    ```bash
    fimod s -i data.json -m https://example.com/normalize.py
    ```

    Shared or remote transforms. Fetched at runtime (requires `full` feature build).

### 🗂️ Mold registries

Registries are named sources — local directories or remote repos — that group molds into collections.
Reference a mold with `@name` (searches all registries in priority order) or `@source/name` (specific registry):

```bash
fimod registry add my ~/molds/
fimod s -i messy.csv -m @my/clean_csv   # resolves to ~/molds/clean_csv.py
fimod s -i messy.csv -m @clean_csv      # searches P0, P1, P2… until found
```

Config stored in `~/.config/fimod/sources.toml`. See [CLI Reference — Mold Registries](cli-reference.md#mold-registries).

### 📁 Directory molds

When `-m` points to a directory, fimod looks for an entry point in this order:

1. `<dir>/__main__.py` — Python package convention
2. `<dir>/<dirname>.py` — script named after the directory

---

## 🦀 Monty

fimod uses [Monty](https://github.com/pydantic/monty), a **Rust implementation of Python's core semantics** from the Pydantic team. No CPython, no FFI, no GIL.

!!! info "Python subset, not CPython"
    Molds run on Monty, not CPython. You get Python syntax, common built-ins, and selected standard-library modules, but not PyPI packages or full stdlib parity. Fimod adds Rust-powered helpers for regex, dot paths, iteration, hashing, templating, logging, and validation.

!!! warning "Monty is early-stage"
    Monty is a very young project. Its API and feature set may change significantly between releases. fimod pins a specific Monty commit, but upgrading may require adapting mold scripts if Monty's behaviour changes.

The Rust-powered helpers (`re_*`, `dp_*`, `it_*`, `hs_*`, `tpl_*`, `msg_*`, `gk_*`) are injected into every mold. Notably, regex built-ins use [fancy-regex](https://github.com/fancy-regex/fancy-regex) syntax, based on Rust's `regex` crate and Oniguruma — **not** Python's `re` module. See [Built-ins Reference](../reference/built-ins.md) for details.

=== "✅ Supported"

    - List/dict comprehensions, ternary expressions (`x if cond else y`)
    - `for` / `while` loops, `if` / `elif` / `else`
    - String methods: `.upper()`, `.strip()`, `.split()`, `.replace()`, `.startswith()`, ...
    - Dict methods: `.get()`, `.keys()`, `.values()`, `.items()`, `.pop()`
    - `len()`, `int()`, `str()`, `float()`, `bool()`, `isinstance()`
    - f-strings: `f"Hello {name}"`, `f"{x:.2f}"`, `f"{x!r}"`
    - Nested functions, multiple return values (tuples)

=== "❌ Not supported"

    - Arbitrary PyPI packages such as `requests`, `pandas`, or `sqlalchemy`
    - Full standard-library parity
    - `del`
    - File I/O, network calls, system access

See [Mold Scripting — Monty Limitations](mold-scripting.md#monty-limitations) for the full list.

---

## 🛡️ Security model

Transforms are **pure functions**: they receive data, manipulate it, and return a result.

!!! success "What mold scripts cannot do"
    - ❌ Read or write files
    - ❌ Make network requests
    - ❌ Access environment variables or the OS
    - ❌ Import external libraries

All I/O happens in Rust, outside the script execution boundary. You can run molds from URLs without sandboxing concerns — the script simply cannot reach outside the data it was given.

---

## 🔧 Intermediate representation

The internal format between pipeline stages is always `serde_json::Value`.

!!! warning "Practical consequences"
    - **CSV values are always strings** — cast in your script: `int(row["age"])`
    - **TOML output requires a root-level object** — arrays/scalars at the root will fail
    - **YAML anchors** are normalized to JSON-compatible equivalents
    - **All formats are interconvertible** — they all map to the same JSON-compatible types
