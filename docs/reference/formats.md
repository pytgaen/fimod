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

# 🔀 NDJSON → compact JSON array
fimod s -i events.ndjson -e 'data' --output-format json-compact

# 🔗 Slurp + NDJSON
cat *.json | fimod s --slurp -e 'data' --output-format ndjson
```

!!! tip "Identity JSON ↔ NDJSON conversions stream natively"
    The exact identity conversion `-e 'data'` streams local/stdin JSON
    top-level arrays directly to NDJSON when the output format resolves to
    `ndjson` or `jsonl`. In the other direction, local/stdin NDJSON streams
    directly to `json` or `json-compact`, ignoring empty lines and preserving
    exact JSON integers. Both paths bypass Monty and keep memory bounded by the
    largest item. Non-array JSON roots keep the normal identity conversion path.
    Eligible regular-file destinations are written through a temporary sibling
    and replaced only after the complete input succeeds; existing Unix file
    permissions are preserved. Existing symbolic links, and existing files on
    Windows, use the buffered identity path to preserve normal file-write
    semantics. When writing to stdout, values emitted before a later parse error
    cannot be retracted, as with any streaming pipe.

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
- **Output**: Serialized from an array of objects. By default, keys of the
  first object become headers. Use `--csv-header` for an explicit output schema
  or `--csv-scan` to scan more rows for columns.

!!! warning "CSV values are always strings"
    Cast in your transform: `int(row["age"])`, `float(row["price"])`

### CSV options

| Option | Description |
|--------|-------------|
| `--csv-delimiter <char>` | Separator character (default: `,`). Use `\t` for TSV. |
| `--csv-output-delimiter <char>` | Separator for output (defaults to `--csv-delimiter`). |
| `--csv-no-input-header` | No header in input — columns named `col0`, `col1`, ... |
| `--csv-no-output-header` | Don't write header row in output. |
| `--csv-header "a,b,c"` | Explicit input column names; also an explicit output schema for object rows. |
| `--csv-scan <N>` | Rows to scan for object-output columns (`1` default, `0` = all rows). |

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

## 📑 Lines

Line-oriented format: each line becomes an array element, and each array element becomes a line. **Never auto-detected** — always use `--input-format lines` or `--output-format lines` explicitly.

- **Input**: Splits on `\n` / `\r\n` → `["line1", "line2", ...]`. Trailing newline does not produce an empty element.
- **Output**: Each array element on its own line — strings are written raw, objects/numbers are serialized as compact JSON. A single string value is written as-is with a trailing newline.

```bash
# 🔤 Uppercase each line (input)
fimod s -i data.txt --input-format lines -e '[l.upper() for l in data]'

# 🔍 Filter lines (input)
fimod s -i app.log --input-format lines -e '[l for l in data if "ERROR" in l]'

# 📦 JSON array → one item per line (output)
fimod s -i names.json -e 'data' --output-format lines

# 🐚 Emit shell-friendly output from any format (output)
fimod s -i users.json -e '[u["email"] for u in data]' --output-format lines
```

!!! tip "Lines vs TXT vs NDJSON"
    - **`txt`**: entire file as a single string — use for free-form text.
    - **`lines`**: one string per line — use for line-by-line processing or shell-friendly output.
    - **`ndjson`**: one **JSON value** per line — use when lines contain structured data.

## 📥 Raw (`--output-format raw`)

An **output-only** format for copying binary streams or raw bytes. It bypasses
the normal parsing, mold, and serialization pipeline completely.

- **Input**: stdin, a local file, or an HTTP(S) URL.
- **Output**: the unchanged byte stream. A single input writes to stdout by
  default or to `-o PATH`. Multiple inputs require `-O`; each file is written
  in the current directory using its input path or URL basename.

```bash
# Copy one local binary without parsing it
fimod s -i archive.bin --output-format raw -o archive-copy.bin

# Copy multiple local files, preserving each basename
fimod s -i first/a.bin second/b.bin --output-format raw -O

# Download using the URL filename
fimod s -i https://example.com/archive.tar.gz --output-format raw -O
```

`--output-format raw` cannot be combined with a mold or expression. The
separate `set_output_format("raw")` mold API can pass through the original HTTP
body and therefore requires an HTTP envelope input.

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

When `-e 'data'` is the only pipeline step, fimod treats it as a native identity
conversion and does not start Monty. Any non-identity expression or multi-step
chain uses the normal mold pipeline.

### JSON → shell-friendly text

A common need: extract values from JSON and feed them to shell tools. Use `--output-format lines` for lists, `--output-format txt` for single values:

```bash
# 📋 List all names, one per line — ready for xargs, while read, etc.
fimod s -i users.json -e '[u["name"] for u in data]' --output-format lines
# Alice
# Bob
# Charlie

# 🐚 Capture a single value into a shell variable (no JSON quotes)
VERSION=$(fimod s -i package.json -e 'data["version"]' --output-format txt)

# 🔗 Pipe to other tools
fimod s -i repos.json -e '[r["ssh_url"] for r in data]' --output-format lines | xargs -I{} git clone {}
```

!!! note "Why not `txt`?"
    `txt` on an array produces a single compact JSON string (`["a","b","c"]`).
    `lines` produces one element per line — much easier to chain with shell tools.
