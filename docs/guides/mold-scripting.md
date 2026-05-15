# 🐍 Mold Scripting

Mold scripts are written in the Python subset supported by [Monty](https://github.com/pydantic/monty), a Rust implementation of Python's core semantics. No system Python required.

## 🎯 The `transform` function

Your script must define a function named `transform` that receives `data` and returns the result. Fimod passes extra context (`args`, `env`, `headers`, `pipeline`) as keyword arguments.

!!! tip "Always keep `**_` in reusable molds"
    `**_` is the recommended convention and should be treated as mandatory for reusable molds. It lets your script ignore context it does not need, and it keeps the mold compatible when fimod adds new keyword parameters.

```python
# Minimal — data only
def transform(data, **_):
    return [item for item in data if item["active"]]

# With args
def transform(data, args, **_):
    return [item for item in data if item["age"] > int(args["min_age"])]

# Full signature — all current parameters
def transform(data, args, env, headers, pipeline, **_):
    # process data
    return data
```

## ⚡ Inline expressions (`-e`)

For quick one-liners, use `-e`. The expression receives `data` and its return value is the output:

```bash
fimod s -i data.json -e '[u for u in data if u["active"]]'
fimod s -i data.json -e '{"count": len(data)}'
```

Multi-statement expressions need an explicit `def transform`:

```bash
fimod s -i data.json -e '
def transform(data, **_):
    result = []
    for item in data:
        result.append(item["name"].upper())
    return result
'
```

## 📦 Data types

Data arrives as standard Python types:

| Type | When |
|------|------|
| `dict` `{}` | JSON objects, YAML/TOML mappings |
| `list` `[]` | JSON arrays, CSV datasets, NDJSON lines |
| `str` | TXT input (raw content), Lines elements |
| `int`, `float`, `bool`, `None` | Primitives |

---

## 🧰 Built-in functions

fimod injects a set of helpers into every mold — **no `import` needed**. See [Built-ins Reference](../reference/built-ins.md) for complete signatures.

### 🔍 Regex (`re_*`)

Powered by [fancy-regex](https://github.com/fancy-regex/fancy-regex) — supports lookahead, lookbehind, backreferences, atomic groups. **Not Python's `re` module** — see [Built-ins Reference](../reference/built-ins.md) for the full syntax differences.

```python
# 📧 Find all email addresses
def transform(data, args, env, headers, **_):
    return re_findall(r"\w+@\w+\.\w+", data["text"])

# 🧹 Clean whitespace
def transform(data, args, env, headers, **_):
    return {"cleaned": re_sub(r"\s+", " ", data["text"])}

# ✂️ Split on multiple delimiters
def transform(data, args, env, headers, **_):
    return re_split(r"[,;]\s*", data["tags"])

# 👤 Lookahead — extract usernames from emails
def transform(data, args, env, headers, **_):
    return re_findall(r"\w+(?=@)", data["text"])

# 📋 Capture groups — extract structured data
def transform(data, args, env, headers, **_):
    m = re_search(r"(?P<user>\w+)@(?P<domain>\w+)", data["email"])
    if m:
        return {"user": m["groups"][0], "domain": m["named"]["domain"]}
    return None

# 🔄 Replacement with group references — Python syntax (\1, \g<name>)
def transform(data, args, env, headers, **_):
    return re_sub(r"(\w+)@(\w+)", r"\2/\1", data["text"])

# 🔄 Named group replacement
def transform(data, args, env, headers, **_):
    return re_sub(r"(?P<user>\w+)@(?P<domain>\w+)", r"\g<domain>/\g<user>", data["text"])

# 🔢 Replace only first N occurrences (count argument)
def transform(data, args, env, headers, **_):
    return re_sub(r"\d+", "X", data["text"], 1)   # replace first match only
```

Available: `re_search` · `re_match` · `re_findall` · `re_sub` · `re_split`

And their `_fancy` counterparts: `re_search_fancy` · `re_match_fancy` · `re_findall_fancy` · `re_sub_fancy` · `re_split_fancy`

!!! note "Two syntaxes for replacements"
    `re_sub` uses **Python `re` syntax**: `\1`, `\2`, `\g<name>`.
    `re_sub_fancy` uses **fancy-regex syntax**: `$1`, `$2`, `${name}`.
    For all other functions (`re_search`, `re_match`, `re_findall`, `re_split`), the `_fancy` variants are identical — provided for API consistency in fancy-mode molds.

### 🗂️ Dotpath (`dp_*`)

Navigate and mutate nested structures without chained dict/array accesses:

```python
def transform(data, args, env, headers, **_):
    city    = dp_get(data, "user.address.city")
    country = dp_get(data, "user.address.country", "unknown")  # with default
    last    = dp_get(data, "items.-1")   # 🔢 negative index = from end
    return {"city": city, "country": country}

# dp_set returns a new deep copy — original unchanged
def transform(data, args, env, headers, **_):
    data = dp_set(data, "meta.processed", True)
    data = dp_set(data, "config.db.host", "localhost")
    return data
```

### 🔁 Iteration helpers (`it_*`)

Convenience functions for common list/dict operations:

```python
# 📂 Group by field name (string, not lambda!)
def transform(data, args, env, headers, **_):
    return it_group_by(data, "department")

# 🔼 Sort by field
def transform(data, args, env, headers, **_):
    return it_sort_by(data, "age")

# 🧹 Deduplicate by field (keeps first occurrence)
def transform(data, args, env, headers, **_):
    return it_unique_by(data, "email")

# 🌀 Recursive flatten: [1, [2, [3, 4]]] → [1, 2, 3, 4]
def transform(data, args, env, headers, **_):
    return it_flatten(data["nested_lists"])

# 🔑 Unique primitives
def transform(data, args, env, headers, **_):
    return it_unique(data["tags"])
```

!!! warning "Field name, not lambda"
    `it_group_by`, `it_sort_by`, and `it_unique_by` take a **field name string** — not a lambda function.

### #️⃣ Hash functions (`hs_*`)

```python
# 🔒 Anonymize PII
def transform(data, args, env, headers, **_):
    for user in data:
        user["email"] = hs_sha256(user["email"])
    return data

# 🔑 Stable ID from composite key
def transform(data, args, env, headers, **_):
    for row in data:
        row["id"] = hs_md5(f"{row['name']}|{row['dob']}")
    return data
```

Available: `hs_md5` · `hs_sha1` · `hs_sha256` — all return lowercase hex strings.

### 📝 Templating (`tpl_*`)

Generate **any text file** from data using [Jinja2](https://jinja.palletsprojects.com/) templates — Dockerfiles, nginx configs, k8s manifests, reports, `.env` files. This extends fimod from data→data to **data→text**.

**Inline templates** with `tpl_render_str` — great for one-liners and small molds:

```python
def transform(data, args, env, headers, **_):
    return tpl_render_str("""
FROM python:{{ python_version }}-slim
{% for pkg in packages %}
RUN pip install {{ pkg }}
{% endfor %}
COPY . /app
CMD {{ cmd | tojson }}
""", data)
```

**File templates** with `tpl_render_from_mold` — for larger templates, keep `.j2` files alongside the mold for clean separation of logic and presentation:

```
my_mold/
├── my_mold.py           # Python logic
└── templates/
    ├── Dockerfile.j2     # Jinja2 template
    └── compose.yaml.j2
```

```python
# my_mold/my_mold.py
"""Generate Dockerfile from project config."""
# fimod: output-format=txt

def transform(data, args, env, headers, **_):
    tpl = args.get("template", "Dockerfile.j2")
    return tpl_render_from_mold(f"templates/{tpl}", data)
```

All Jinja2 features are available: loops, conditions, filters (`upper`, `join`, `tojson`, `default`, `selectattr`, …), macros, and `{% break %}`/`{% continue %}`. Dict key order is preserved.

!!! tip
    Combine with `--output-format txt` (or `# fimod: output-format=txt` in mold defaults) so the rendered text is written as-is, without JSON quoting.

Available: `tpl_render_str(template, ctx)` · `tpl_render_from_mold(path, ctx)` — see [Built-ins Reference](../reference/built-ins.md#template-functions-tpl_) for `auto_escape` option and the [Quick Tour](quick-tour.md#datatext-jinja2-templating) for more examples.

### 📢 Message logging (`msg_*`)

Output diagnostic messages to stderr — useful for progress, warnings, and debugging without polluting stdout:

```python
def transform(data, args, env, headers, **_):
    msg_info(f"Processing {len(data)} records")
    for row in data:
        if not row.get("email"):
            msg_warn("Record missing email: " + str(row.get("id")))
    return data
```

Available: `msg_print` (no prefix) · `msg_info` (`[INFO]`) · `msg_warn` (`[WARN]`) · `msg_error` (`[ERROR]`)

### 🛡️ Validation gates (`gk_*`)

Assert conditions and fail the pipeline with a non-zero exit code:

```python
def transform(data, args, env, headers, **_):
    gk_assert(data.get("version"), "missing 'version' field")
    gk_warn(len(data.get("items", [])) > 0, "items list is empty")
    if data.get("coverage", 0) < 80:
        gk_fail(f"Coverage {data['coverage']}% below 80% threshold")
    return data
```

Available: `gk_fail(msg)` · `gk_assert(cond, msg)` · `gk_warn(cond, msg)` — see [Built-ins Reference](../reference/built-ins.md#gatekeeper-functions-gk_) for truthiness rules.

### 🛡️ Sandbox-gated stdlib calls

A few stdlib calls that read host state are gated by the [sandbox policy](cli-reference.md#sandbox-policy). They raise `PermissionError` when the policy denies them — catch it if you want a graceful fallback.

| Call | Gate | Default |
|------|------|---------|
| `datetime.now()` | `allow_clock = true` | denied |
| `date.today()` | `allow_clock = true` | denied |
| `os.getenv(KEY)` | `allow_env` glob matches `KEY` | denied (empty list) |
| `os.environ` | *(always)* | denied |
| `pathlib.Path` I/O | *(always)* | denied |

```python
# Clock — requires allow_clock = true
from datetime import datetime
def transform(data, **_):
    data["stamped_at"] = datetime.now().isoformat()
    return data

# Env — requires "LANG" to match an allow_env pattern
import os
def transform(data, **_):
    try:
        data["locale"] = os.getenv("LANG")
    except PermissionError:
        data["locale"] = None
    return data
```

Bootstrap the canonical sandbox file with `fimod setup sandbox defaults --yes`, then edit it to grant the clock / env keys your molds need. For ad-hoc runs, pass `--sandbox-file <path>` to point at a specific policy, or `--sandbox-file=""` to force zero-authorization.

### 🔄 Environment substitution

`env_subst(template, dict)` replaces `${VAR}` placeholders using a dict:

```python
def transform(data, args, env, headers, **_):
    return env_subst("https://${HOST}:${PORT}/api", env)
```

```bash
fimod s -i data.json --env 'HOST,PORT' -e 'env_subst("${HOST}:${PORT}", env)' --output-format txt
```

### 🚦 Exit control

`set_exit(code)` sets the process exit code without stopping execution:

```python
def transform(data, args, env, headers, **_):
    if not data.get("valid"):
        set_exit(1)
    return data
```

When combined with `--check`, `set_exit` takes priority for the exit code — see [Exit Codes](../reference/exit-codes.md).

---

## 📊 CSV `headers` global

When the input is CSV with a header row, fimod injects a `headers` global (list of column names in file order):

```python
def transform(data, args, env, headers, **_):
    # headers = ["name", "age", "email"]  ← auto-injected by fimod
    return {"columns": headers, "count": len(data)}

# 🔢 Generic numeric column processing
def transform(data, args, env, headers, **_):
    numeric_cols = [h for h in headers if h.endswith("_amount")]
    for row in data:
        row["total"] = sum(float(row[c]) for c in numeric_cols)
    return data
```

!!! note
    `headers` is only available when the input has a header row. Not injected with `--csv-no-input-header`.

---

## 🔗 The `pipeline` parameter

`pipeline` gives a mold live access to the step chain it is running inside. Declare it only when you need it:

```python
def transform(data, pipeline, **_):
    return pipeline.length()   # number of steps in the chain
```

### Introspection

`pipeline.current_step()` returns the current step as a handle. Fields are read via `step.get('key')` — direct attribute access (`step.index`) and indexing (`step['index']`) are **not** supported (single API surface).

| Key | Type | Description |
|-----|------|-------------|
| `'index'` | `int` | 0-based position in the chain |
| `'input_format'` | `str | None` | Effective input format |
| `'output_format'` | `str | None` | Output format override |
| `'input'` | `str | None` | Input file path |
| `'output'` | `str | None` | Output file path |
| `'in_place'` | `bool` | `--in-place` flag |
| `'slurp'` | `bool` | `--slurp` flag |
| `'no_input'` | `bool` | `--no-input` flag |
| `'args'` | `dict` | Merged args (CLI ∪ `Step.create.args`, spec wins) for the current step; spec args (or `{}`) for a future step |

```python
def transform(data, pipeline, **_):
    step = pipeline.current_step()
    return {
        "step":  step.get('index'),
        "total": pipeline.length(),
        "fmt":   step.get('output_format'),
    }
```

`pipeline.step(i)` accesses any step by absolute index — current or future, never past:

```bash
# Middle step of a 3-step chain reads its own index
fimod s -i data.json -e data -e "pipeline.current_step().get('index')" -e data
# → 1
```

Unknown keys raise `Step.get('<key>'): unknown field`.

### Mutating the current step

Use `step.set(key, value)` to mutate a step. Writable keys:

| Key | Value type | Effect | Where |
|-----|-----------|--------|-------|
| `'exit'` | `int` | Set process exit code | current or future |
| `'output_format'` | `str` | Override output serialization format | current or future |
| `'input_format'` | `str` | Force re-parse before the next step | current or future |
| `'output_file'` | `str` | Override output file path | current or future |
| `'args'` | `dict` | Replace the entire args block (CLI `--arg` merge still applied at exec) | **future only** |

```python
def transform(data, pipeline, **_):
    if not data.get("valid"):
        pipeline.current_step().set('exit', 1)
    return data
```

Works on future steps too — the mutation is queued and applied just before the target step runs:

```python
pipeline.step(2).set('output_format', 'yaml')
pipeline.step(2).set('args', {'threshold': 100})
```

!!! note "Global functions still available"
    `set_exit()`, `set_output_format()`, `set_output_file()` remain available as global functions.

### Injecting steps dynamically

Add new steps to the running chain with `Step.create(expr=...)` (inline expression) or `Step.create(mold=...)` (mold reference):

```python
def transform(data, pipeline, **_):
    # Insert a step immediately after this one
    pipeline.insert_next(Step.create(expr="data[:100]"))
    pipeline.insert_next(Step.create(mold="@my-registry/sort"))

    # Append at the end of the chain
    pipeline.append(Step.create(expr="len(data)"))
    return data
```

`Step.create()` arguments:

| Arg | Required | Description |
|-----|----------|-------------|
| `expr=` | one of | Inline expression (like `-e`) |
| `mold=` | one of | Mold reference (like `-m`) |
| `input_format=` | no | Force input format for the injected step |
| `output_format=` | no | Set output format for the injected step |
| `args=` | no | Per-step args dict propagated to the injected mold (heterogeneous types: bool, int, nested dicts). Merged with CLI `--arg` at exec time — spec values win on conflict. |

```python
pipeline.append(Step.create(
    mold="@my-registry/normalize",
    args={"strict": True, "limits": {"rows": 1000}},
))
```

!!! note "Snapshot semantics for `pipeline.length()` and `pipeline.step(i)`"
    Both are computed at the start of each step. A step injected by step `i`
    via `insert_next` / `append` is **only visible from step `i+1` onwards** —
    inside the same mold run, `pipeline.length()` and `pipeline.step(j)` still
    reflect the chain as it was when the current step started. Plan injections
    accordingly: you cannot read or mutate a step you've just appended in the
    same `transform()` call.

---

## ⚙️ Mold defaults

Scripts can embed default CLI options via `# fimod:` directives at the very top of the file:

```python
# fimod: input-format=csv, output-format=json
# fimod: csv-delimiter=;
def transform(data, args, env, headers, **_):
    return [{"name": row["name"], "age": int(row["age"])} for row in data]
```

!!! tip "CLI wins by default — `!=` locks a directive"
    Explicit CLI arguments override mold defaults declared with `=`.
    Use `!=` to force a value the caller cannot override (e.g. `output-format!=yaml`).

See [Mold Defaults](../reference/mold-defaults.md) for all supported directives.

---

## 📎 The `args` dict

`--arg name=value` populates the `args` parameter of `transform(data, args, **_)`:

```python
def transform(data, args, **_):
    limit  = int(args["threshold"])
    prefix = args.get("prefix", "")
    return [u for u in data if u["name"].startswith(prefix) and u["age"] > limit]
```

```bash
fimod s -i users.json -m filter.py --arg threshold=30 --arg prefix="A"
```

When no `--arg` is passed, `args` is an empty dict `{}`.

---

## ✅ Available Python features

- [x] List/dict comprehensions
- [x] Ternary expressions (`x if cond else y`)
- [x] String methods: `.upper()`, `.strip()`, `.split()`, `.replace()`, `.startswith()`, ...
- [x] Dict methods: `.get()`, `.keys()`, `.values()`, `.items()`, `.pop()`
- [x] `for` / `while` loops, `if` / `elif` / `else`
- [x] `in` / `not in` operators
- [x] `isinstance()`, `len()`, `int()`, `str()`, `float()`, `bool()`
- [x] f-strings (`f"Hello {name}"`, `f"{x:.2f}"`, `f"{x!r}"`)
- [x] Nested functions, multiple return values (tuples)
- [x] All built-in helpers (`re_*`, `re_*_fancy`, `dp_*`, `it_*`, `hs_*`, `tpl_*`, `msg_*`, `gk_*`, `env_subst`, `set_exit`, `set_input_format`, `set_output_format`, `set_output_file`, `Step.create(...)`)

## ❌ Monty limitations

- [ ] `import` — only `re`, `math`, `datetime`, `json`, `sys`, `typing`, `asyncio`, `pathlib`, `os` (partial)
- [ ] `del` statement
- [ ] File I/O, network, system calls
