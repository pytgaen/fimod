# ⚙️ Mold Defaults

A mold script can embed default CLI options via `# fimod:` comment directives. This makes scripts **self-describing** — callers don't need to remember which format or options a script expects.

!!! info "Where to put directives"
    Directives must appear at the **very top** of the file, before any code.

```python
# fimod: input-format=csv, output-format=json
# fimod: csv-delimiter=;
def transform(data, **_):
    return [{"name": row["name"], "age": int(row["age"])} for row in data]
```

Running this with just `fimod s -i data.csv -m script.py` automatically uses `;` as the delimiter.

---

## 📋 Supported directives

| Directive | Type | Description |
|-----------|------|-------------|
| `input-format=<fmt>` | value | Input format (`json`, `ndjson`, `yaml`, `toml`, `csv`, `txt`, `lines`) |
| `output-format=<fmt>` | value | Output format |
| `csv-delimiter=<char>` | value | CSV input delimiter character |
| `csv-output-delimiter=<char>` | value | CSV output delimiter character |
| `csv-header=<cols>` | value | Explicit CSV column names (comma-separated) |
| `csv-no-input-header` | flag | Input has no header row |
| `csv-no-output-header` | flag | Don't write header in output |
| `no-follow` | flag | Don't follow HTTP redirects |
| `arg=<name>[:<type>[?][=<default>]] [desc]` | value | Document and optionally type an `--arg` parameter |
| `env=<VAR> [desc]` | value | Document an `--env` variable (name + optional description) |

---

## Typed Args

Plain `arg=<name>` keeps the historical behavior: it documents the parameter,
and `--arg name=value` reaches the mold as a string.

Add a type to validate and cast before `transform()` runs:

```python
# fimod: arg=threshold:int "Minimum score"
# fimod: arg=dry_run:bool?=false
# fimod: arg=filter:json?
def transform(data, args, **_):
    if args["dry_run"]:
        return data
    threshold = args["threshold"]
    return [row for row in data if row["score"] >= threshold]
```

Supported V1 types:

| Type | Example input | Value in `args` |
|------|---------------|-----------------|
| `str` | `--arg name=alice` | string |
| `int` | `--arg threshold=30` | integer |
| `float` | `--arg ratio=0.75` | float |
| `bool` | `--arg dry_run=true` | boolean |
| `json` | `--arg filter={"active":true}` | JSON value |

Required and optional forms:

```python
# required; missing arg fails before mold execution
# fimod: arg=threshold:int

# optional; missing arg is absent from args
# fimod: arg=threshold:int?

# optional with default; missing arg is injected
# fimod: arg=threshold:int?=10

# explicit None default
# fimod: arg=threshold:int?=None
```

`?` only allows the argument to be omitted. If the caller provides a value, it
must still parse as the declared type. Runtime pipeline args created with
`Step.create(args={...})` are validated by the target mold's directives after
they are merged with CLI args.

## 📏 Priority and parsing rules

!!! success "CLI wins by default"
    Explicit CLI arguments always override mold defaults declared with `=`.

- Directives are read from the **top of the file** — scanning stops at the first non-comment, non-blank line
- Multiple key-value pairs can appear on the **same `# fimod:` line**, comma-separated
- Unknown keys are **silently ignored**
- Directives in `-e` inline expressions are **never** parsed

---

## 🔒 Forced directives (`!=`)

Use `!=` instead of `=` to declare a directive as **forced** — the CLI cannot override it.

```python
# fimod: output-format!=yaml
def transform(data, **_):
    return data
```

```bash
# The mold forces YAML regardless of what the caller passes
fimod s -i data.json -m strict.py --output-format json
# → output is YAML, not JSON
```

This is useful for molds that produce a format incompatible with others (e.g., a mold that always emits YAML for a downstream tool).

!!! tip "Debug info"
    Run with `--debug` to see which directives are forced:
    ```
    [debug] mold forces output-format=yaml
    ```

Forced and default directives can be mixed on the same mold:

```python
# fimod: input-format!=csv, output-format=json
```

Here `input-format` is locked to `csv`; `output-format` is a suggestion that the CLI can override.

---

## 💡 Complete example

```python
# fimod: input-format!=csv
# fimod: output-format=json
# fimod: csv-delimiter=;
# fimod: csv-no-input-header
def transform(data, args, env, headers, **_):
    # data = [{"col0": ..., "col1": ...}, ...]
    return [{"name": row["col0"], "value": int(row["col1"])} for row in data]
```

```bash
# ✅ Use defaults — input-format is locked to csv
fimod s -i data.csv -m script.py

# 🔀 Override output format at call time (allowed — it's a default, not forced)
fimod s -i data.csv -m script.py --output-format yaml

# ⚠️  --input-format is ignored — the mold forces csv
fimod s -i data.csv -m script.py --input-format json
```
