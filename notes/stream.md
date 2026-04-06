# `--stream` Mode

## Motivation

Working with CSV in fimod is verbose (row-oriented by default):
```
fimod s -i a.csv -e "[r['c3'] for r in data]"
```
Streaming solves this naturally: `data` becomes a single row, so `data['c3']` just works.

Also enables processing large files without loading everything into memory.

## Prerequisite fix: array-of-arrays CSV output

`serialize_csv` should support rows that are arrays (not just objects). If `rows[0]` is an array → write fields directly, no header (or use `--csv-header-names` if provided). Small, independent fix.

## Design

A single `--stream` flag enables unit-by-unit processing. No new subcommand or mode — fimod inspects the mold to adapt behavior.

### What is a "unit"

| Format | `data` received by `transform` |
|---|---|
| CSV | `{"c1": "A", "c2": "1"}` (dict from row) |
| NDJSON | the JSON object/value of the line |
| TXT/lines | the raw line (string) |

Applies to line-oriented formats only. Tree formats (JSON, YAML, TOML) require full loading by nature.

### Simple stream

`-e` or `transform(data, **_)` — each unit in, each unit out:
```bash
# CSV: extract a column
fimod s -i big.csv --stream -e "data['c3']"

# CSV: filter rows
fimod s -i big.csv --stream -e "data if int(data['c2']) > 2 else None"

# TXT: uppercase each line
fimod s -i file.txt --stream -e "data.upper()"

# TXT: filter lines containing ERROR
fimod s -i log.txt --stream -e "data if 'ERROR' in data else None"
```

### Stateful stream

Fimod detects `ctx` in `transform`'s signature and/or the presence of `initialize`/`finalize`.

| Function | When | Receives | Returns |
|---|---|---|---|
| `initialize(headers, **_)` | Before first unit | CSV headers (or `None`) | the initial `ctx` |
| `transform(data, ctx, **_)` | Each unit | unit + ctx | value to emit (or `None` to skip) |
| `finalize(ctx, **_)` | After last unit | final ctx | value to emit (or `None`) |

All three are optional. Detection rules:
- `initialize` present → called, its return becomes `ctx`
- `initialize` absent → `ctx = {}`
- `finalize` absent → nothing after last unit
- `transform` without `ctx` in signature → simple stream, no state

### Example: sum a CSV column

```python
"""Sum c2 values"""

def initialize(headers, **_):
    return {"total": 0}

def transform(data, ctx, **_):
    ctx["total"] += int(data["c2"])
    return None  # don't emit rows

def finalize(ctx, **_):
    return {"total": ctx["total"]}
```

### Example: count non-empty lines

```python
def initialize(**_):
    return {"count": 0}

def transform(data, ctx, **_):
    if data.strip():
        ctx["count"] += 1
    return None

def finalize(ctx, **_):
    return ctx["count"]
```

### Output behavior

Stream output is flushed unit-by-unit (natural for NDJSON, CSV, TXT/lines). `finalize` output is emitted as a final entry on stdout. For aggregation-only molds (all `transform` return `None`), only `finalize` output appears.

### Scope

Aligns with ROADMAP.md #12 (Large files streaming, P3).
