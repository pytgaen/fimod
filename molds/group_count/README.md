# group_count

Group records by a field and count occurrences. Results are sorted by count (ascending).

## Usage

```bash
fimod s -i logs.json -m @group_count --arg field=status
```

## Example

**Input** (`logs.json`):
```json
[
  {"status": "200", "path": "/api/users"},
  {"status": "404", "path": "/api/missing"},
  {"status": "200", "path": "/api/items"},
  {"status": "200", "path": "/api/orders"},
  {"status": "500", "path": "/api/crash"}
]
```

**Output**:
```json
[
  {"value": "404", "count": 1},
  {"value": "500", "count": 1},
  {"value": "200", "count": 3}
]
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `field` | Yes | Field name to group by |
