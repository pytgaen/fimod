# checkpoint

Fail with a non-zero exit code if data is empty at this point in the pipeline.
Data is passed through unchanged so downstream steps still receive it.

**Requires fimod ≥ 0.6.0** (uses the `pipeline` parameter and `msg_error` built-in).

## Usage

```bash
# Fail if the API response is empty
fimod s -i response.json -m @checkpoint

# In a multi-step pipeline — fail at step 2 with a descriptive label
fimod s -i response.json \
  -m extract.py \
  -m @checkpoint --arg label='after extract' \
  -m publish.py
```

## Example

**Input** — empty list:
```json
[]
```

**Output** (exit code 1, data passed through):
```json
[]
```

Stderr:
```
[ERROR] [step 1/1] checkpoint: data is empty
```

**Input** — non-empty:
```json
[{"id": 1}]
```

**Output** (exit code 0):
```json
[{"id": 1}]
```

## Args

| Arg | Required | Default | Description |
|-----|----------|---------|-------------|
| `label` | No | `checkpoint` | Description shown in the error message |
| `exit_code` | No | `1` | Exit code when data is empty |

## What counts as empty

- `null`
- empty string `""`
- empty list `[]`
- empty dict `{}`
