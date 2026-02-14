# rename_keys

Rename JSON keys via a mapping. Useful to adapt between APIs or naming conventions.

## Usage

```bash
fimod s -i data.json -m @rename_keys --arg mapping=firstName:first_name,lastName:last_name
```

## Example

**Input** (`data.json`):
```json
[
  {"firstName": "Alice", "lastName": "Smith", "age": 30},
  {"firstName": "Bob", "lastName": "Jones", "age": 25}
]
```

**Output**:
```json
[
  {"first_name": "Alice", "last_name": "Smith", "age": 30},
  {"first_name": "Bob", "last_name": "Jones", "age": 25}
]
```

Keys not in the mapping are preserved unchanged.

### Single object

```bash
fimod s -i record.json -m @rename_keys --arg mapping=user_id:id,created_at:date
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `mapping` | Yes | Comma-separated `old_key:new_key` pairs |
