# jq_compat

Common jq operations (`get`, `map`, `select`) via `--arg` parameters. A bridge for jq users who don't want to write Python expressions.

## Usage

### Extract a nested value (`jq '.user.address.city'`)

```bash
fimod s -i data.json -m @jq_compat --arg get=user.address.city
```

**Input**: `{"user": {"address": {"city": "Paris"}}}` **Output**: `"Paris"`

### Map a field (`jq '[.[].name]'`)

```bash
fimod s -i users.json -m @jq_compat --arg map=name
```

**Input**: `[{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]` **Output**: `["Alice", "Bob"]`

### Filter by value (`jq '[.[] | select(.active == "true")]'`)

```bash
fimod s -i users.json -m @jq_compat --arg select="active=true"
```

**Input**: `[{"name": "Alice", "active": "true"}, {"name": "Bob", "active": "false"}]` **Output**: `[{"name": "Alice", "active": "true"}]`

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `get` | No | Dotpath to extract a nested value |
| `map` | No | Field name to extract from each object in an array |
| `select` | No | `field=value` filter to keep matching objects |

Only one arg should be used at a time. Priority: `get` > `map` > `select`.
