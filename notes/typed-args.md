# Typed Mold Args

- **Status**: implemented
- **Target**: current development cycle
- **Source**: `../archive/human-2026-05-20.md`

## Problem

`--arg threshold=30` historically reached molds as a string. Every mold that
needed numbers, booleans, or structured values had to repeat parsing and
validation:

```python
threshold = int(args["threshold"])
```

That is noisy in simple molds and easy to get wrong in shared registry molds.

## Goal

Let molds declare expected argument types through the existing `# fimod:`
directive mechanism, then validate and cast before calling `transform()`.

## Syntax

The directive keeps the existing `key=value` shape:

```python
# fimod: arg=threshold:int "Minimum score"
# fimod: arg=dry_run:bool?=false
# fimod: arg=filter:json?

def transform(data, args, **_):
    if args["dry_run"]:
        return data
    return [row for row in data if row["score"] >= args["threshold"]]
```

Grammar:

```text
arg=<name>[:<type>[?][=<default>]] [description]
```

Plain `arg=<name> [description]` stays documentation-only and preserves the old
string behavior.

## Types

| Type | Input examples | Result |
|---|---|---|
| `str` | `name=alice` | string |
| `int` | `threshold=30` | integer |
| `float` | `ratio=0.75` | float |
| `bool` | `dry_run=true`, `dry_run=false` | boolean |
| `json` | `filter={"a":1}` | JSON value |

Undeclared args remain strings for compatibility.

## Optionality And Defaults

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

`?` means the argument may be omitted. It does not turn invalid values into
`None`; `--arg threshold=abc` still fails for `threshold:int?`.

Defaults are only accepted on optional typed args. `None` is the explicit
Python/Monty null default. If the mold wants a string `"None"`, use a string
typed arg with a quoted default in a future default syntax extension; this is
not a V1 requirement.

## Validation Behavior

Validation happens after CLI args and runtime pipeline args are merged, and
before mold execution.

- CLI `--arg` values start as strings and are cast by the target mold's typed
  declarations.
- `Step.create(args={...})` values may already be typed JSON/Monty values; the
  target mold still validates them.
- Step args continue to win on key conflicts with CLI args.
- `pipeline.current_step().get("args")` sees the same cast dict that
  `transform(..., args=...)` receives.

Example failure:

```text
arg error: threshold expected int, got "abc"
```

## Non-Goals

- Full schema language.
- Nested typed collections beyond `json`.
- Type inference from default values.
- Changing existing behavior for undeclared args.
- Adding `--arg-json`.
