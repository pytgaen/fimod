# flatten_nested

Flatten a nested JSON object into dot-path keys. Useful for flattening configurations, preparing data for CSV exports, or diffing nested structures.

## Usage

```bash
fimod s -i config.json -m @flatten_nested
```

## Example

**Input** (`config.json`):
```json
{
  "database": {
    "host": "localhost",
    "port": 5432,
    "credentials": {"user": "admin", "pass": "secret"}
  },
  "debug": true
}
```

**Output**:
```json
{
  "database.host": "localhost",
  "database.port": 5432,
  "database.credentials.user": "admin",
  "database.credentials.pass": "secret",
  "debug": true
}
```

### Arrays are indexed

```json
{"tags": ["a", "b"]}
```
becomes:
```json
{"tags.0": "a", "tags.1": "b"}
```
