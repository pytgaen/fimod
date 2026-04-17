# filter_fields

Keep or drop fields of an object (or array of objects) using **dotpaths**. Supports nested fields — where [`pick_fields`](../pick_fields/README.md) only matches top-level keys.

## Usage

```bash
# Drop fields (including nested paths)
fimod s -i users.json -m @filter_fields --arg mode=drop --arg fields=password,meta.debug

# Keep only specific fields (including nested paths)
fimod s -i users.json -m @filter_fields --arg mode=keep --arg fields=id,user.email
```

## Example — drop

**Input** (`api.json`):
```json
{
  "id": 42,
  "user": {"name": "Alice", "email": "a@x.io", "internal_id": "x-1"},
  "meta": {"debug": true, "version": "1.0"}
}
```

```bash
fimod s -i api.json -m @filter_fields --arg mode=drop --arg fields=user.internal_id,meta.debug
```

**Output**:
```json
{
  "id": 42,
  "user": {"name": "Alice", "email": "a@x.io"},
  "meta": {"version": "1.0"}
}
```

## Example — keep

```bash
fimod s -i api.json -m @filter_fields --arg mode=keep --arg fields=id,user.email
```

**Output**:
```json
{
  "id": 42,
  "user": {"email": "a@x.io"}
}
```

## Array of objects

Applied element-wise:

```bash
fimod s -i users.json -m @filter_fields --arg mode=drop --arg fields=password
# [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `mode` | No (default `keep`) | `keep` or `drop` — exits with code 1 on any other value |
| `fields` | Yes | Comma-separated list of dotpaths. Empty → returns data unchanged |

## When to use

- **`filter_fields`** — nested paths, or you want drop semantics.
- **`pick_fields`** — top-level keys only, simpler syntax.
