# ⚙️ Mold Defaults

A mold script can embed default CLI options via `# fimod:` comment directives. This makes scripts **self-describing** — callers don't need to remember which format or options a script expects.

!!! info "Where to put directives"
    Directives must appear at the **very top** of the file, before any code.

```python
# fimod: input-format=csv, output-format=json
# fimod: csv-delimiter=;
def transform(data, args, env, headers):
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
| `csv-header=<cols>` | value | Explicit column names (comma-separated) |
| `csv-no-input-header` | flag | Input has no header row |
| `csv-no-output-header` | flag | Don't write header in output |
| `raw-mode=<mode>` | value | Raw output mode: `no-quote` (strings without JSON quotes) or `binary` (raw bytes) |
| `description=<text>` | value | One-line description shown by `fimod mold list` |

---

## 📏 Priority and parsing rules

!!! success "CLI always wins"
    Explicit CLI arguments always override mold defaults.

- Directives are read from the **top of the file** — scanning stops at the first non-comment, non-blank line
- Multiple key-value pairs can appear on the **same `# fimod:` line**, comma-separated
- Unknown keys are **silently ignored**
- Directives in `-e` inline expressions are **never** parsed

---

## 💡 Complete example

```python
# fimod: input-format=csv
# fimod: output-format=json
# fimod: csv-delimiter=;
# fimod: csv-no-input-header
# fimod: output-format=json-compact
def transform(data, args, env, headers):
    # data = [{"col0": ..., "col1": ...}, ...]
    return [{"name": row["col0"], "value": int(row["col1"])} for row in data]
```

```bash
# ✅ Use defaults
fimod s -i data.csv -m script.py

# 🔀 Override output format at call time
fimod s -i data.csv -m script.py --output-format yaml
```
