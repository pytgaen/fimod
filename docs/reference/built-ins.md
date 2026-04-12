# 🧰 Built-ins Reference

fimod injects the following functions and globals into every mold — **no `import` needed**.

---

## 🔍 Regex functions (`re_*`)

Powered by [fancy-regex](https://github.com/fancy-regex/fancy-regex) — supports PCRE2 features: lookahead, lookbehind, backreferences, atomic groups.

**Two functions for replacements** — `re_sub` uses Python syntax (`\1`, `\g<name>`); `re_sub_fancy` uses fancy-regex syntax (`$1`, `${name}`).

!!! note "Pattern syntax differences from Python `re`"
    Only the **replacement** syntax differs by mode. The pattern syntax always uses fancy-regex:

    - **Named groups**: `(?P<name>...)` (same as Python) or `(?<name>...)`
    - **Advanced features**: atomic groups `(?>...)`, possessive quantifiers `a++` — not in Python `re`
    - **Flags**: inline `(?i)`, `(?m)`, `(?s)` — no separate `re.IGNORECASE` etc.

### Match result format

`re_search` and `re_match` return a dict (or `None`):

```python
{
    "match":  "full match text",
    "start":  0,     # byte offset
    "end":    5,     # byte offset
    "groups": ["group1", "group2", ...],  # numbered capture groups (1..N)
    "named":  {"name": "value", ...}      # named groups, or None if no named groups
}
```

`groups` is always present (empty list if no capture groups). `named` is `None` unless the pattern uses `(?P<name>...)`.

### Function reference

| Function | Signature | Returns |
|----------|-----------|---------|
| `re_search` | `re_search(pattern, text)` | Match dict (see above) or `None` |
| `re_match` | `re_match(pattern, text)` | Match dict or `None` — anchored to start of text |
| `re_findall` | `re_findall(pattern, text)` | Python-style: see below |
| `re_sub` | `re_sub(pattern, replacement, text [, count])` | `str` — Python syntax: `\1`, `\g<name>` |
| `re_sub_fancy` | `re_sub_fancy(pattern, replacement, text [, count])` | `str` — fancy-regex syntax: `$1`, `${name}` |
| `re_split` | `re_split(pattern, text)` | `[str, ...]` — captured groups included |

### `re_findall` — Python-style group behaviour

| Pattern groups | Returns | Example |
|----------------|---------|---------|
| No groups | `["match1", "match2", ...]` | `re_findall(r"\d+", "a1b2")` → `["1", "2"]` |
| 1 group | `["group1_val", ...]` | `re_findall(r"(\d+)@", "1@2@")` → `["1", "2"]` |
| N groups | `[["g1", "g2"], ...]` | `re_findall(r"(\w+)=(\d+)", "a=1 b=2")` → `[["a","1"], ["b","2"]]` |

### `re_sub` / `re_sub_fancy` — count and syntax

Both functions replace all occurrences by default. Pass an optional `count` to limit substitutions.

```python
re_sub(r"\d+", "X", "a1b2c3")       # → "aXbXcX"  (all)
re_sub(r"\d+", "X", "a1b2c3", 1)    # → "aXb2c3"  (first only)
```

**`re_sub`** — Python `re` syntax in replacements:

```python
re_sub(r"(\w+)@(\w+)", r"\2/\1", "user@host")              # → "host/user"
re_sub(r"(?P<u>\w+)@(?P<d>\w+)", r"\g<d>/\g<u>", "a@b")   # → "b/a"
```

**`re_sub_fancy`** — fancy-regex syntax (`$1`, `${name}`):

```python
re_sub_fancy(r"(\w+)@(\w+)", "$2/$1", "user@host")         # → "host/user"
re_sub_fancy(r"(\w+)@(\w+)", "$2/$1", "user@host", 1)      # first only
```

### `re_split` — captured groups included

When the pattern has capture groups, captured text is included in the result (same as Python `re.split`):

```python
re_split(r"([,;])\s*", "a, b;c")    # → ["a", ",", "b", ";", "c"]
re_split(r"[,;]\s*", "a, b;c")      # → ["a", "b", "c"]  (no groups = no extras)
```

---

## 🗂️ Dotpath functions (`dp_*`)

Navigate and mutate nested structures using dot-separated paths.

| Function | Signature | Returns |
|----------|-----------|---------|
| `dp_get` | `dp_get(data, path)` | Value at path, or `None` if not found |
| `dp_get` | `dp_get(data, path, default)` | Value at path, or `default` if not found |
| `dp_set` | `dp_set(data, path, value)` | New deep copy of `data` with `value` at `path` |

### Path syntax

| Segment | Meaning | Example |
|---------|---------|---------|
| Text | dict key | `"user.address.city"` |
| Integer | array index | `"items.0"` (first), `"items.-1"` (last) |

!!! tip
    Missing intermediate keys or out-of-range indices return `None` for `dp_get`. `dp_set` creates missing intermediate keys automatically.

---

## 🔁 Iteration helpers (`it_*`)

Convenience functions for common list/dict operations not natively supported by Monty.

| Function | Signature | Returns |
|----------|-----------|---------|
| `it_keys` | `it_keys(dict)` | List of keys |
| `it_values` | `it_values(dict)` | List of values |
| `it_flatten` | `it_flatten(array)` | Recursively flattened list |
| `it_group_by` | `it_group_by(array, key)` | Dict of lists, grouped by field name (insertion order) |
| `it_sort_by` | `it_sort_by(array, key)` | Sorted list by field name (stable sort) |
| `it_unique` | `it_unique(array)` | Deduplicated list (first occurrence kept) |
| `it_unique_by` | `it_unique_by(array, key)` | Deduplicated by field name (first occurrence kept) |

!!! warning "Field name, not lambda"
    `it_group_by`, `it_sort_by`, and `it_unique_by` take a **field name string** — not a lambda.

!!! info "it_flatten is recursive"
    `[1, [2, [3, 4]]]` → `[1, 2, 3, 4]`

---

## #️⃣ Hash functions (`hs_*`)

| Function | Signature | Returns |
|----------|-----------|---------|
| `hs_md5` | `hs_md5(text)` | MD5 hex digest (lowercase) |
| `hs_sha1` | `hs_sha1(text)` | SHA-1 hex digest (lowercase) |
| `hs_sha256` | `hs_sha256(text)` | SHA-256 hex digest (lowercase) |

All functions accept a single string and return a lowercase hex string.

---

## 📝 Template functions (`tpl_*`)

Data→text generation using [Jinja2](https://jinja.palletsprojects.com/) templates (via [MiniJinja](https://github.com/mitsuhiko/minijinja)). Extends Fimod's data→data pipeline to data→text for generating configs, reports, Dockerfiles, k8s manifests, etc.

| Function | Signature | Returns |
|----------|-----------|---------|
| `tpl_render_str` | `tpl_render_str(template, ctx, auto_escape=False)` | Rendered string |
| `tpl_render_from_mold` | `tpl_render_from_mold(path, ctx, auto_escape=False)` | Rendered string |

**`tpl_render_str(template, ctx, auto_escape=False)`** — Render a Jinja2 template string with a context dict. All built-in Jinja2 filters (`upper`, `join`, `tojson`, …), loops, conditions, and macros are available.

```python
def transform(data, args, env, headers):
    return tpl_render_str("""
FROM python:{{ python_version }}-slim
{% for pkg in packages %}
RUN uv pip install {{ pkg }}
{% endfor %}
""", data)
```

```bash
echo '{"python_version":"3.12","packages":["flask","requests"]}' \
  | fimod s -e 'tpl_render_str("Hello {{ name }}!", data)' --output-format txt
```

**`tpl_render_from_mold(path, ctx, auto_escape=False)`** — Load a `.j2` file relative to the mold's directory and render it. Works with directory molds and registry molds. Enables clean separation of logic (Python) and presentation (Jinja2).

```python
# my_mold/my_mold.py
def transform(data, args, env, headers):
    tpl = args.get("template", "Dockerfile.j2")
    return tpl_render_from_mold(f"templates/{tpl}", data)
```

```bash
fimod s -i data.json -m ./my_mold/ --output-format txt
```

!!! note
    `tpl_render_from_mold` requires a file-based or registry mold — it cannot be used with inline expressions (`-e`). Path traversal outside the mold directory is blocked for security.

Set `auto_escape=True` when generating HTML to automatically escape `<`, `>`, `&`, etc.

---

## 📢 Message functions (`msg_*`)

Output diagnostic messages to **stderr** without affecting the data pipeline. All functions take a single string and return `None`.

Which functions produce output depends on the `--quiet` / `--msg-level` flags:

| Function | Stderr output | Visible by default | `--quiet` | `--msg-level=verbose` | `--msg-level=trace` |
|----------|---------------|--------------------|:---------:|:---------------------:|:-------------------:|
| `msg_print` | `text` | ✓ | — | ✓ | ✓ |
| `msg_info` | `[INFO] text` | ✓ | — | ✓ | ✓ |
| `msg_warn` | `[WARN] text` | ✓ | — | ✓ | ✓ |
| `msg_error` | `[ERROR] text` | ✓ | ✓ | ✓ | ✓ |
| `msg_verbose` | `[VERBOSE] text` | — | — | ✓ | ✓ |
| `msg_trace` | `[TRACE] text` | — | — | — | ✓ |

```python
def transform(data, args, env, headers):
    msg_verbose(f"Input has {len(data)} records")
    missing = [r for r in data if not r.get("email")]
    if missing:
        msg_warn(f"{len(missing)} records without email")
    msg_trace(f"First record: {data[0]}")
    return data
```

---

## 🛡️ Gatekeeper functions (`gk_*`)

Validation helpers for asserting conditions and controlling pipeline failure. Work with `set_exit()` — `gk_fail` and `gk_assert` set exit code to 1.

| Function | Signature | Behavior |
|----------|-----------|----------|
| `gk_fail` | `gk_fail(msg)` | Emit `[ERROR] msg` to stderr, set exit code to 1 |
| `gk_assert` | `gk_assert(condition, msg)` | If `condition` is falsy → `gk_fail(msg)` |
| `gk_warn` | `gk_warn(condition, msg)` | If `condition` is falsy → `[WARN] msg` to stderr (no exit) |

`gk_assert` and `gk_warn` use **Python-style truthiness**: `None`, `False`, `0`, `0.0`, `""`, `[]` are falsy.

```python
def transform(data, args, env, headers):
    gk_assert(data.get("version"), "missing 'version' field")
    gk_warn(len(data.get("items", [])) > 0, "items list is empty")
    if data.get("coverage", 0) < 80:
        gk_fail(f"Coverage {data['coverage']}% below 80% threshold")
    return data
```

!!! tip
    The mold continues executing after `gk_fail` / `gk_assert` — this lets you collect multiple errors in one run. The exit code is set to 1 at process exit.

---

## 🔄 Environment substitution (`env_subst`)

| Function | Signature | Returns |
|----------|-----------|---------|
| `env_subst` | `env_subst(template, dict)` | `str` — template with `${VAR}` placeholders replaced |

Unknown variables are left as-is (standard `envsubst` behavior). Only `${VAR}` syntax is supported (`$VAR` without braces is not substituted).

```python
def transform(data, args, env, headers):
    url = env_subst("https://${HOST}:${PORT}/api", env)
    return {"url": url, "data": data}
```

```bash
fimod s -i data.json --env 'HOST,PORT' -m inject_url.py
```

---

## 🚦 Exit control

| Function | Signature | Returns |
|----------|-----------|---------|
| `set_exit` | `set_exit(code)` | `None` |

Sets the process exit code from inside a mold. `code` is an integer 0–255. The mold continues executing to completion after the call.

See [Exit Codes](exit-codes.md) for the interaction with `--check`.

---

## 🔀 Format control

| Function | Signature | Returns |
|----------|-----------|---------|
| `set_input_format` | `set_input_format(name)` | `None` |
| `cast_input_format` | `cast_input_format(name, value)` | `value` |
| `set_output_format` | `set_output_format(name)` | `None` |

**`set_input_format(name)`** — re-parses the output of the current step as the given format before feeding it as input to the next step. Useful with `--input-format http` to re-parse a string body as JSON, CSV, etc.

**`cast_input_format(name, value)`** — same as `set_input_format` but returns `value`. Useful as a single-expression one-liner when both the format hint and the return value are needed.

**`set_output_format(name)`** — overrides the final output format (like a dynamic `--output-format`). Also accepts `"raw"` for binary pass-through (HTTP downloads).

```bash
# Fetch raw HTTP response, then re-parse body as JSON
fimod s -i https://jsonplaceholder.typicode.com/todos/1 \
    --input-format http \
    -e 'cast_input_format("json", data["body"])' \
    -e 'data["title"]' --output-format txt
```

Supported format names for `set_input_format`: `json`, `ndjson`, `yaml`, `toml`, `csv`, `txt`, `lines`, `http`.
`set_output_format` additionally accepts `"raw"` (binary pass-through, requires `--input-format http`).

---

## 🧩 Transform parameters

All mold scripts receive four parameters: `def transform(data, args, env, headers)`.

### `args`

Dict of `--arg name=value` pairs. Empty dict `{}` when no `--arg` is passed:

```python
def transform(data, args, env, headers):
    limit  = int(args["threshold"])
    prefix = args.get("prefix", "")   # with default
    return [u for u in data if u["name"].startswith(prefix) and u["age"] > limit]
```

### `env`

Dict of filtered environment variables. Populated by `--env PATTERN` (glob patterns, comma-separated, repeatable). Empty dict `{}` when no `--env` is passed:

```bash
fimod s -i data.json --env 'HOME,USER' -e 'env["HOME"]'
fimod s -i data.json --env 'GITHUB_*' -e 'env'
fimod s -i data.json --env '*' -e 'env.get("CI", "false")'
```

### `headers`

List of CSV column names when the input format is **CSV with a header row**. `None` for non-CSV input or when using `--csv-no-input-header`:

```csv
name,score,passed
Alice,87,true
Bob,42,false
Carol,95,true
```

```python
def transform(data, args, env, headers):
    # headers = ["name", "score", "passed"] for CSV, None otherwise
    if headers and "score" in headers:
        return it_sort_by(data, "score")
    return data
```

```bash
fimod s -i grades.csv -m sort_by_score.py
```
