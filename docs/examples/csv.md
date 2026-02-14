# CSV — Practical Examples

## Output: Generate CSV from a list

### From a list of dicts (header auto-derived from keys)

The CSV header is taken from the dict keys. Column order follows insertion order.

```bash
echo '[{"name":"Alice","age":30},{"name":"Bob","age":25}]' | \
  fimod s -e 'data' --output-format csv
```
```
name,age
Alice,30
Bob,25
```

### Controlling the header (column names and order)

Return dicts with the exact keys you want. The key names become column headers.

```bash
fimod s -i https://jsonplaceholder.typicode.com/users \
  -e '[{"id": u["id"], "name": u["name"], "email": u["email"], "city": dp_get(u, "address.city")} for u in data]' \
  -o contacts.csv
```

### Without header row (`--csv-no-output-header`)

Useful for appending to an existing file or feeding into tools that expect raw rows.

```bash
echo '[{"name":"Alice","age":30},{"name":"Bob","age":25}]' | \
  fimod s -e 'data' --output-format csv --csv-no-output-header
```
```
Alice,30
Bob,25
```

---

## Input: CSV with header

Default behavior — the first row is treated as the header. Each row becomes a dict keyed by column name.

```bash
printf 'name,age\nAlice,30\nBob,25\n' | \
  fimod s -e 'data' --input-format csv --output-format json
```
```json
[
  {"name": "Alice", "age": "30"},
  {"name": "Bob", "age": "25"}
]
```

### Casting values

All values come in as strings. Cast explicitly when needed:

```bash
printf 'name,age\nAlice,30\nBob,25\n' | \
  fimod s -e '[{**row, "age": int(row["age"])} for row in data]' \
  --input-format csv --output-format json
```

```json
[
  {"name": "Alice", "age": 30},
  {"name": "Bob", "age": 25}
]
```

---

## Input: CSV without header

### Using `--csv-header` to name the columns

When the file has no header row, use `--csv-header` to declare column names.
Rows arrive as dicts with those names.

```bash
printf 'Alice,30\nBob,25\n' | \
  fimod s -e 'data' --input-format csv --csv-header "name,age" --output-format json
```

```json
[
  {"name": "Alice", "age": "30"},
  {"name": "Bob", "age": "25"}
]
```

### Using `--csv-no-input-header` (columns as lists)

When no header is declared, rows arrive as plain lists (`["Alice", "30"]`).
Access columns by index.

```bash
printf 'Alice,30\nBob,25\n' | \
  fimod s -e '[{"name": r[0], "age": int(r[1])} for r in data]' \
  --input-format csv --csv-no-input-header --output-format json
```

```json
[
  {"name": "Alice", "age": 30},
  {"name": "Bob", "age": 25}
]
```

---

## Custom delimiter (TSV and others)

### Tab-separated input

```bash
printf 'name\tage\nAlice\t30\nBob\t25\n' | \
  fimod s --input-format csv --csv-delimiter '\t' -e 'data' --output-format json
```

```json
[
  {"name": "Alice", "age": "30"},
  {"name": "Bob", "age": "25"}
]
```

### Semicolon-separated input (common in European locales)

```bash
printf 'name;age\nAlice;30\nBob;25\n' | \
  fimod s --input-format csv --csv-delimiter ';' -e 'data' --output-format json
```

```json
[
  {"name": "Alice", "age": "30"},
  {"name": "Bob", "age": "25"}
]
```

### Different input and output delimiters

```bash
printf 'name\tage\nAlice\t30\nBob\t25\n' | \
  fimod s --input-format csv --csv-delimiter '\t' -e 'data' \
  --output-format csv --csv-output-delimiter ';'
```

```
name;age
Alice;30
Bob;25
```
