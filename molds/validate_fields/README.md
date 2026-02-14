# validate_fields

Validate that required fields exist in the input data. Exits with code 1 if any field is missing — useful in CI pipelines.

## Usage

```bash
fimod s -i config.json -m @validate_fields --arg required=database.host,database.port,app.name
```

## Example

**Input** (`config.json`):
```json
{"database": {"host": "localhost"}, "app": {"name": "myapp"}}
```

**Output** (exit code 1):
```json
{"valid": false, "missing": ["database.port"]}
```

If all fields are present, output is `{"valid": true, "missing": []}` with exit code 0.

### Use in CI

```bash
# Fail the pipeline if required config keys are missing
fimod s -i config.json -m @validate_fields --arg required=db.host,db.port,secret_key \
  && echo "Config OK" || echo "Missing fields!"
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `required` | Yes | Comma-separated list of dotpaths that must be present |
