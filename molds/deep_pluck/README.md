# deep_pluck

Extract nested fields by dotpath into a flat object. The last segment of each path becomes the output key.

## Usage

```bash
fimod s -i users.json -m @deep_pluck --arg paths=user.name,user.address.city
```

## Example

**Input** (`users.json`):
```json
[
  {"user": {"name": "Alice", "address": {"city": "Paris", "zip": "75001"}}},
  {"user": {"name": "Bob", "address": {"city": "Lyon", "zip": "69001"}}}
]
```

**Output**:
```json
[
  {"name": "Alice", "city": "Paris"},
  {"name": "Bob", "city": "Lyon"}
]
```

Works on a single object too:

```bash
fimod s -i config.json -m @deep_pluck --arg paths=database.host,database.port
# → {"host": "localhost", "port": 5432}
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `paths` | Yes | Comma-separated dotpaths to extract (e.g. `user.name,user.email`) |
