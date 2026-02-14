# anonymize_pii

Hash specified fields with SHA-256 for anonymization. Works on a single object or an array of objects.

## Usage

```bash
fimod s -i users.json -m @anonymize_pii --arg fields=email,phone
```

## Example

**Input** (`users.json`):
```json
[
  {"name": "Alice", "email": "alice@example.com", "phone": "555-1234"},
  {"name": "Bob", "email": "bob@example.com", "phone": "555-5678"}
]
```

**Output**:
```json
[
  {"name": "Alice", "email": "c160f8cc7a883...", "phone": "b6b11e..."},
  {"name": "Bob", "email": "4b9bb80620f03...", "phone": "a274b0..."}
]
```

Only the specified fields are hashed; all other fields are preserved unchanged.

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `fields` | Yes | Comma-separated list of field names to anonymize |
