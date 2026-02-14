# dedup_by

Deduplicate records by a field, keeping the first occurrence of each value.

## Usage

```bash
fimod s -i events.json -m @dedup_by --arg field=event_id
```

## Example

**Input** (`events.json`):
```json
[
  {"event_id": "a1", "type": "click"},
  {"event_id": "b2", "type": "scroll"},
  {"event_id": "a1", "type": "click"}
]
```

**Output**:
```json
[
  {"event_id": "a1", "type": "click"},
  {"event_id": "b2", "type": "scroll"}
]
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `field` | Yes | Field name to deduplicate on |
