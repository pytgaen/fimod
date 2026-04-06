# 🔀 Formats

fimod supports seamless conversion between multiple formats. Formats are **auto-detected from file extensions**, or set explicitly with `--input-format` / `--output-format`.

---

## 📋 JSON (`.json`)

- **Input**: Parsed as standard JSON.
- **Output**: Serialized as pretty-printed JSON.
- **Structure**: Can be Object `{}`, Array `[]`, or any JSON value.

---

## 📃 NDJSON (`.ndjson`, `.jsonl`)

Newline-Delimited JSON — one JSON value per line. Ideal for streaming data, logs, and event records.

- **Input**: each non-empty line is parsed as a JSON value; result is an **array** of those values.
- **Output**: each array element is serialized as compact JSON on its own line (trailing newline). A non-array result is a single line.

```bash
# 🔍 Filter an NDJSON log
fimod s -i events.ndjson -e '[e for e in data if e["level"] == "error"]'

# 🔀 JSON array → NDJSON
fimod s -i users.json -e 'data' --output-format ndjson

# 🔗 Slurp + NDJSON
cat *.json | fimod s --slurp -e 'data' --output-format ndjson
```

!!! info "NDJSON vs Lines"
    `ndjson` parses each line as JSON. `lines` treats each line as a raw string.

---

## 📝 YAML (`.yaml`, `.yml`)

- **Input**: Parsed into JSON-compatible structure. Anchors and custom types are normalized.
- **Output**: Serialized as YAML.

---

## ⚙️ TOML (`.toml`)

- **Input**: Parsed into JSON-compatible structure.
- **Output**: Serialized as TOML.

!!! warning "TOML requires a root-level object"
    Arrays or scalars at the root will fail — TOML spec constraint.

---

## 📊 CSV (`.csv`, `.tsv`)

- **Input**: Parsed as an **array of objects**. Each row becomes a dict where keys are column headers.
- **Output**: Serialized from an array of objects. Keys of the first object become headers.

!!! warning "CSV values are always strings"
    Cast in your transform: `int(row["age"])`, `float(row["price"])`

### CSV options

| Option | Description |
|--------|-------------|
| `--csv-delimiter <char>` | Separator character (default: `,`). Use `\t` for TSV. |
| `--csv-output-delimiter <char>` | Separator for output (defaults to `--csv-delimiter`). |
| `--csv-no-input-header` | No header in input — columns named `col0`, `col1`, ... |
| `--csv-no-output-header` | Don't write header row in output. |
| `--csv-header "a,b,c"` | Explicit column names (implies no header in file). |

!!! info "Column order is preserved"
    No alphabetical sorting through transforms.

### `headers` global

When the input has a header row, a `headers` global (list of column names in file order) is injected automatically. Not available with `--csv-no-input-header`.

Mold defaults can also set CSV options — see [Mold Defaults](mold-defaults.md).

---

## 📄 TXT (`.txt`, `.text`)

- **Input**: `data` is a **raw string** — the entire file content, as-is.
- **Output**: String values are written raw. Non-string values are serialized as compact JSON.

```bash
# 🔤 data is a string
fimod s -i notes.txt -e 'data.strip().upper()'

# 📦 Return non-string → compact JSON
fimod s -i notes.txt -e '{"length": len(data)}'
```

---

## 📑 Lines (`--input-format lines`)

Splits input into an array of strings, one per line. **Never auto-detected** — use `--input-format lines` explicitly.

- **Input**: `["line1", "line2", ...]`. Handles `\n` and `\r\n`. Trailing newline does not produce an empty element.
- **Output**: each element on its own line. Objects/dicts serialized as compact JSON (NDJSON).

```bash
# 🔤 Uppercase each line
fimod s -i data.txt --input-format lines -e '[l.upper() for l in data]'

# 🔍 Filter lines
fimod s -i app.log --input-format lines -e '[l for l in data if "ERROR" in l]'

# 📦 JSON array → one item per line
fimod s -i names.json -e 'data' --output-format lines
```

## 📥 Raw (`--output-format raw`)

An **output-only** format for downloading binary streams or raw bytes. Bypasses the normal data serialization pipeline completely.

- **Input**: Not supported.
- **Output**: The raw byte stream (e.g. from an HTTP response payload). Requires `--input-format http`.

---

## 🔥 HTTP (`--input-format http`)

By default, `-i https://...` fetches the URL and parses the body directly
(format auto-detected from `Content-Type`). Use `--input-format http` when
you need more than the body: **status codes, response headers, redirects,
or conditional logic on the response**.

`data` is then a dict with the full response envelope:

```python
data = {
    "status": 200,
    "headers": {"content-type": "application/json", ...},
    "body": "...",          # raw string
    "body_size": 1234,      # response size in bytes
    "content_type": "application/json",
    "url": "https://example.com/api/data"
}
```

```bash
# Inspect redirect target
fimod s -i https://github.com/pytgaen/fimod/releases/latest \
    --input-format http --no-follow \
    -e 'data["status"]' --output-format txt
# → 302

# With --input-format http, data["body"] is a raw string — re-parse it explicitly
fimod s -i https://jsonplaceholder.typicode.com/users \
    --input-format http \
    -e 'set_input_format("json"); data["body"]' \
    -e 'len(data)'
```

!!! warning "Input-only"
    `--output-format http` is not supported. HTTP is never auto-detected from extensions.

---

## 🎯 Output format resolution

When `--output-format` is not specified, fimod resolves the output format automatically using this cascade:

| Priority | Source | Example |
|----------|--------|---------|
| 1 | `--output-format` flag | `--output-format json` → JSON |
| 2 | Output file extension | `-o result.yaml` → YAML |
| 3 | Same as input format | Input is CSV → output is CSV |

!!! tip "You don't need `--output-format` when the format stays the same"
    ```bash
    # JSON in → JSON out (no flag needed)
    fimod s -i data.json -e '[x for x in data if x["active"]]'

    # CSV in → CSV out (no flag needed)
    fimod s -i users.csv -e '[r for r in data if r["role"] == "admin"]'

    # Lines in → lines out (no flag needed)
    env | fimod s --input-format lines -e '[l for l in data if "PATH" in l]'
    ```

    Only use `--output-format` when **converting** between formats without an output file.

!!! note "`--no-input` defaults to JSON"
    With `--no-input`, there is no input format to inherit — the output defaults to JSON.

---

## 🔀 Format conversion

Convert between formats using a pass-through expression and either an output file extension or `--output-format`:

```bash
fimod s -i config.yaml -e 'data' -o config.toml          # extension → TOML
fimod s -i data.csv -e 'data' --output-format json        # explicit → JSON
fimod s -i users.json -e 'data' --output-format lines     # explicit → lines
```
