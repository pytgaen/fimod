# auto_anonymize

Append `@anonymize_pii` downstream **only if** the CSV input contains sensitive columns. The mold inspects the CSV header (injected by fimod as the `headers` parameter) and decides at runtime whether anonymisation is needed — no flag to wire externally, no caller-side knowledge of the export schema required.

**Requires fimod ≥ 0.6.0** (uses `pipeline.append(Step.create(args=...))` and the `headers` parameter).

## Usage

```bash
# Default: looks for an 'email' column
fimod s -i users.csv -m @auto_anonymize

# Custom detection list
fimod s -i users.csv -m @auto_anonymize --arg detect=email,phone,ssn -o users.json
```

## Example

**Input** (`users.csv`):
```csv
id,name,email,age
1,Alice,alice@example.com,30
2,Bob,bob@example.com,25
```

```bash
fimod s -i users.csv -m @auto_anonymize
```

**Output** (emails hashed via SHA-256):
```csv
id,name,email,age
1,Alice,ff8d9819fc0e12bf0d24892e45987e249a28dce836a85cad60e28eaaa8c6d976,30
2,Bob,5ff860bf1190596c7188ab851db691f0f3169c453936e9e1eba2f9a47f7a0018,25
```

If the same command is run on a CSV **without** an `email` column (e.g. `id,name,age`), the chain passes through unchanged — `@anonymize_pii` is not appended.

## Args

| Arg | Required | Default | Description |
|-----|----------|---------|-------------|
| `detect` | No | `email` | Comma-separated columns to look for. Any subset present in the header is forwarded as the `fields` arg of `@anonymize_pii`. |

## Notes

- **CSV-only by design**: `headers` is `None` when the input is JSON/YAML/TOML, so non-CSV inputs always pass through unchanged.
- **Detection is set-based**, not order-based. `--arg detect=email,phone,ssn` triggers `@anonymize_pii` if **any** of the three columns is present, and forwards only the columns that are actually there.
- **No-op when nothing matches**: returns data unchanged and does nothing. Safe to leave in a pipeline that occasionally processes non-PII exports.

## When to use

- Same anonymisation policy applied to heterogeneous CSV exports (some carry PII, some don't) without per-export configuration.
- CI / scheduled jobs where the schema may change over time.
- Demonstrates the **conditional routing** pattern of fimod 0.6.0 — see [Dynamic Molds](../../docs/guides/dynamic-molds.md).
