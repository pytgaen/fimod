# Mold Gallery

A curated selection of molds from the examples registry, showcasing what fimod can do out of the box. All of these are available immediately after running `fimod registry setup`.

---

## ⚙️ DevOps & Config

### `@yaml_merge` — Patch a YAML file

Applies a set of `path=value` assignments to an existing YAML document using dot-path notation. Useful for patching Kubernetes manifests or any structured config without a full template engine.

```bash
# Scale a deployment and tag it for production
fimod s -i deployment.yaml -m @yaml_merge \
  --arg set="spec.replicas=3,metadata.labels.env=prod" \
  -o deployment.yaml
```

Values are auto-cast: `true`/`false` become booleans, numeric strings become integers.

```yaml
# Before
spec:
  replicas: 1
metadata:
  labels: {}

# After
spec:
  replicas: 3
metadata:
  labels:
    env: prod
```

### `@env_to_dotenv` — Config to `.env` format

Flattens a JSON/YAML config into a `.env` file with `KEY=value` lines.

```bash
fimod s -i config.yaml -m @env_to_dotenv -o .env
```

### `@validate_fields` — Assert required fields exist

Checks that a set of dotpaths are present in the input. Exits with code 1 if any are missing — perfect for CI gates.

```bash
fimod s -i config.json -m @validate_fields --arg required=database.host,database.port,api.key
```

---

## 🔍 Data Exploration

### `@jq_compat` — Common jq operations

A lightweight jq-compatible interface for the three most-used operations: `get` (dot-path extraction), `map` (field projection), and `select` (filtering). Useful when you want the ergonomics of jq without the dependency.

```bash
# Extract a nested field
fimod s -i data.json -m @jq_compat --arg get=user.address.city

# Project a single field from an array
fimod s -i users.json -m @jq_compat --arg map=name

# Filter by field value
fimod s -i users.json -m @jq_compat --arg select=active=true
```

### `@deep_pluck` — Extract nested fields into a flat object

Pulls values from deep dotpaths and returns a flat key/value object.

```bash
fimod s -i order.json -m @deep_pluck --arg paths=customer.name,shipping.address.city,total
# → {"customer.name": "Alice", "shipping.address.city": "Paris", "total": 99}
```

### `@flatten_nested` — Flatten a nested object to dot-path keys

```bash
fimod s -i config.json -m @flatten_nested
# {"database": {"host": "localhost", "port": 5432}} → {"database.host": "localhost", "database.port": 5432}
```

### `@json_schema_extract` — Extract a simplified JSON schema

Infers a schema from a JSON document — useful for quick documentation or validation scaffolding.

```bash
fimod s -i data.json -m @json_schema_extract
```

---

## 🔄 Data Transformation

### `@pick_fields` — Keep only specified fields

```bash
fimod s -i users.json -m @pick_fields --arg fields=name,email
```

### `@rename_keys` — Rename keys via mapping

```bash
fimod s -i data.json -m @rename_keys --arg mapping=firstName:first_name,lastName:last_name
```

### `@dedup_by` — Deduplicate records by a field

```bash
fimod s -i contacts.json -m @dedup_by --arg field=email
```

### `@group_count` — Group by field and count

```bash
fimod s -i events.json -m @group_count --arg field=type
# → [{"type": "click", "count": 42}, {"type": "view", "count": 128}]
```

### `@split_tags` — Split a delimiter-separated field into a list

```bash
fimod s -i articles.json -m @split_tags --arg field=tags
# "rust,cli,tools" → ["rust", "cli", "tools"]
```

### `@sort_json_keys` — Recursively sort JSON keys

```bash
fimod s -i messy.json -m @sort_json_keys -o clean.json
```

---

## 🛡️ Data Quality

### `@anonymize_pii` — Hash PII fields

Replaces the values of specified fields with their SHA-256 hash. Works on both arrays of objects and single objects.

```bash
fimod s -i users.json -m @anonymize_pii --arg fields=email,phone -o users_anon.json
```

```json
// Before
[{"id": 1, "email": "alice@example.com", "phone": "555-0100"}]

// After
[{"id": 1, "email": "a4c2e...", "phone": "f91b3..."}]
```

The original `id` and any non-listed fields are preserved. Deterministic hashing means the same input always produces the same fingerprint — useful for consistent pseudonymisation across datasets.

---

## 📊 CSV

### `@csv_stats` — Basic statistics on numeric columns

```bash
fimod s -i data.csv -m @csv_stats
# → {"salary": {"min": 30000, "max": 120000, "mean": 65000, "count": 50}, ...}
```

### `@csv_to_json_records` — CSV to JSON array of objects

```bash
fimod s -i data.csv -m @csv_to_json_records -o data.json
```

---

## 📝 Text & Markdown

### `@badge_md` — Generate a shields.io badge

```bash
fimod s -i status.json -m @badge_md --arg label=build --arg status=passing --arg color=green
# → [![build](https://img.shields.io/badge/build-passing-green)]()
```

### `@git_changelog` — Markdown changelog from structured data

```bash
fimod s -i commits.json -m @git_changelog -o CHANGELOG.md
```

### `@markdown_toc` — Extract table of contents from Markdown

```bash
fimod s -i README.md -m @markdown_toc
```

### `@log_parse` — Parse log lines into structured records

Extracts structured fields from log lines using regex capture groups.

```bash
fimod s -i app.log -m @log_parse
```

---

## 🗄️ BigQuery

### `@bq_insert` — Generate INSERT statement from `bq show`

```bash
bq show --format=json project:dataset.table | fimod s -m @bq_insert
```

### `@bq_select` — Generate SELECT statement from `bq show`

```bash
bq show --format=json project:dataset.table | fimod s -m @bq_select
```

---

## 🚀 fimod-powered registry

More production-ready molds are available in the [fimod-powered](https://github.com/pytgaen/fimod-powered) registry:

```bash
fimod registry add fimod-powered https://github.com/pytgaen/fimod-powered
```

| Mold | Description |
|------|-------------|
| `@gh_latest` | Fetch latest GitHub release tag, resolve asset download URLs |
| `@download` | wget-like file download with binary pass-through |
| `@poetry_migrate` | Migrate Poetry 1.x `pyproject.toml` to uv or Poetry 2 |
| `@skylos_to_gitlab` | Convert Skylos dead code reports to GitLab Code Quality JSON |

> See the [fimod-powered README](https://github.com/pytgaen/fimod-powered) for the full list.
