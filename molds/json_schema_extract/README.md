# json_schema_extract

Extract a simplified JSON schema from a document. Recursively replaces values with their type names.

## Usage

```bash
fimod s -i sample.json -m @json_schema_extract
```

## Example

**Input** (`sample.json`):
```json
{
  "name": "Alice",
  "age": 30,
  "active": true,
  "score": 9.5,
  "tags": ["dev", "admin"],
  "address": {"city": "Paris", "zip": 75001}
}
```

**Output**:
```json
{
  "name": "string",
  "age": "integer",
  "active": "boolean",
  "score": "number",
  "tags": "list",
  "address": {"city": "string", "zip": "integer"}
}
```

Useful for understanding the shape of an unfamiliar JSON document or generating documentation stubs.
