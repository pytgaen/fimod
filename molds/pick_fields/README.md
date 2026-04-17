# pick_fields

Keep only specified fields from an object or an array of objects. All other fields are dropped.

> Need nested paths or **drop** semantics? See [`filter_fields`](../filter_fields/README.md).

## Usage

```bash
fimod s -i users.json -m @pick_fields --arg fields=id,name,email
```

## Example

**Input** (`users.json`):
```json
[
  {"id": 1, "name": "Alice", "email": "alice@example.com", "password": "hashed", "role": "admin"},
  {"id": 2, "name": "Bob", "email": "bob@example.com", "password": "hashed", "role": "user"}
]
```

**Output**:
```json
[
  {"id": 1, "name": "Alice", "email": "alice@example.com"},
  {"id": 2, "name": "Bob", "email": "bob@example.com"}
]
```

### Single object

```bash
fimod s -i config.json -m @pick_fields --arg fields=host,port
# {"host": "localhost", "port": 8080}
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `fields` | Yes | Comma-separated list of field names to keep |
