# 🐍 Mold Scripting

Mold scripts are written in the Python subset supported by [Monty](https://github.com/pydantic/monty), a Rust implementation of Python's core semantics. No system Python required.

## 🎯 The `transform` function

Your script must define a function named `transform` that accepts `data` and returns the result:

```python
def transform(data, args, env, headers):
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
def transform(data, args, env, headers):
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
def transform(data, args, env, headers):
    return re_findall(r"\w+@\w+\.\w+", data["text"])

# 🧹 Clean whitespace
def transform(data, args, env, headers):
    return {"cleaned": re_sub(r"\s+", " ", data["text"])}

# ✂️ Split on multiple delimiters
def transform(data, args, env, headers):
    return re_split(r"[,;]\s*", data["tags"])

# 👤 Lookahead — extract usernames from emails
def transform(data, args, env, headers):
    return re_findall(r"\w+(?=@)", data["text"])

# 📋 Capture groups — extract structured data
def transform(data, args, env, headers):
    m = re_search(r"(?P<user>\w+)@(?P<domain>\w+)", data["email"])
    if m:
        return {"user": m["groups"][0], "domain": m["named"]["domain"]}
    return None

# 🔄 Replacement with group references — Python syntax (\1, \g<name>)
def transform(data, args, env, headers):
    return re_sub(r"(\w+)@(\w+)", r"\2/\1", data["text"])

# 🔄 Named group replacement
def transform(data, args, env, headers):
    return re_sub(r"(?P<user>\w+)@(?P<domain>\w+)", r"\g<domain>/\g<user>", data["text"])

# 🔢 Replace only first N occurrences (count argument)
def transform(data, args, env, headers):
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
def transform(data, args, env, headers):
    city    = dp_get(data, "user.address.city")
    country = dp_get(data, "user.address.country", "unknown")  # with default
    last    = dp_get(data, "items.-1")   # 🔢 negative index = from end
    return {"city": city, "country": country}

# dp_set returns a new deep copy — original unchanged
def transform(data, args, env, headers):
    data = dp_set(data, "meta.processed", True)
    data = dp_set(data, "config.db.host", "localhost")
    return data
```

### 🔁 Iteration helpers (`it_*`)

Convenience functions for common list/dict operations:

```python
# 📂 Group by field name (string, not lambda!)
def transform(data, args, env, headers):
    return it_group_by(data, "department")

# 🔼 Sort by field
def transform(data, args, env, headers):
    return it_sort_by(data, "age")

# 🧹 Deduplicate by field (keeps first occurrence)
def transform(data, args, env, headers):
    return it_unique_by(data, "email")

# 🌀 Recursive flatten: [1, [2, [3, 4]]] → [1, 2, 3, 4]
def transform(data, args, env, headers):
    return it_flatten(data["nested_lists"])

# 🔑 Unique primitives
def transform(data, args, env, headers):
    return it_unique(data["tags"])
```

!!! warning "Field name, not lambda"
    `it_group_by`, `it_sort_by`, and `it_unique_by` take a **field name string** — not a lambda function.

### #️⃣ Hash functions (`hs_*`)

```python
# 🔒 Anonymize PII
def transform(data, args, env, headers):
    for user in data:
        user["email"] = hs_sha256(user["email"])
    return data

# 🔑 Stable ID from composite key
def transform(data, args, env, headers):
    for row in data:
        row["id"] = hs_md5(f"{row['name']}|{row['dob']}")
    return data
```

Available: `hs_md5` · `hs_sha1` · `hs_sha256` — all return lowercase hex strings.

### 📝 Templating (`tpl_*`)

Generate **any text file** from data using [Jinja2](https://jinja.palletsprojects.com/) templates — Dockerfiles, nginx configs, k8s manifests, reports, `.env` files. This extends fimod from data→data to **data→text**.

**Inline templates** with `tpl_render_str` — great for one-liners and small molds:

```python
def transform(data, args, env, headers):
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

def transform(data, args, env, headers):
    tpl = args.get("template", "Dockerfile.j2")
    return tpl_render_from_mold(f"templates/{tpl}", data)
```

All Jinja2 features are available: loops, conditions, filters (`upper`, `join`, `tojson`, `default`, `selectattr`, …), macros, and `{% break %}`/`{% continue %}`. Dict key order is preserved.

!!! tip
    Combine with `--output-format txt` (or `# fimod: output-format=txt` in mold defaults) so the rendered text is written as-is, without JSON quoting.

Available: `tpl_render_str(template, ctx)` · `tpl_render_from_mold(path, ctx)` — see [Built-ins Reference](../reference/built-ins.md#template-functions-tpl) for `auto_escape` option and the [Quick Tour](quick-tour.md#data-text-jinja2-templating) for more examples.

### 📢 Message logging (`msg_*`)

Output diagnostic messages to stderr — useful for progress, warnings, and debugging without polluting stdout:

```python
def transform(data, args, env, headers):
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
def transform(data, args, env, headers):
    gk_assert(data.get("version"), "missing 'version' field")
    gk_warn(len(data.get("items", [])) > 0, "items list is empty")
    if data.get("coverage", 0) < 80:
        gk_fail(f"Coverage {data['coverage']}% below 80% threshold")
    return data
```

Available: `gk_fail(msg)` · `gk_assert(cond, msg)` · `gk_warn(cond, msg)` — see [Built-ins Reference](../reference/built-ins.md#gatekeeper-functions-gk) for truthiness rules.

### 🔄 Environment substitution

`env_subst(template, dict)` replaces `${VAR}` placeholders using a dict:

```python
def transform(data, args, env, headers):
    return env_subst("https://${HOST}:${PORT}/api", env)
```

```bash
fimod s -i data.json --env 'HOST,PORT' -e 'env_subst("${HOST}:${PORT}", env)' --output-format txt
```

### 🚦 Exit control

`set_exit(code)` sets the process exit code without stopping execution:

```python
def transform(data, args, env, headers):
    if not data.get("valid"):
        set_exit(1)
    return data
```

When combined with `--check`, `set_exit` takes priority for the exit code — see [Exit Codes](../reference/exit-codes.md).

---

## 📊 CSV `headers` global

When the input is CSV with a header row, fimod injects a `headers` global (list of column names in file order):

```python
def transform(data, args, env, headers):
    # headers = ["name", "age", "email"]  ← auto-injected by fimod
    return {"columns": headers, "count": len(data)}

# 🔢 Generic numeric column processing
def transform(data, args, env, headers):
    numeric_cols = [h for h in headers if h.endswith("_amount")]
    for row in data:
        row["total"] = sum(float(row[c]) for c in numeric_cols)
    return data
```

!!! note
    `headers` is only available when the input has a header row. Not injected with `--csv-no-input-header`.

---

## ⚙️ Mold defaults

Scripts can embed default CLI options via `# fimod:` directives at the very top of the file:

```python
# fimod: input-format=csv, output-format=json
# fimod: csv-delimiter=;
def transform(data, args, env, headers):
    return [{"name": row["name"], "age": int(row["age"])} for row in data]
```

!!! tip "CLI always wins"
    Explicit CLI arguments always override mold defaults.

See [Mold Defaults](../reference/mold-defaults.md) for all supported directives.

---

## 📎 The `args` dict

`--arg name=value` populates the `args` parameter of `transform(data, args, env, headers)`:

```python
def transform(data, args, env, headers):
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
- [x] All built-in helpers (`re_*`, `re_*_fancy`, `dp_*`, `it_*`, `hs_*`, `tpl_*`, `msg_*`, `gk_*`, `env_subst`, `set_exit`, `set_input_format`, `set_output_format`, `set_output_file`)

## ❌ Monty limitations

- [ ] `import` — no stdlib, no modules
- [ ] `del` statement
- [ ] File I/O, network, system calls
