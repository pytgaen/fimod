# compact_if_big

Switch the pipeline output to **compact JSON** (single-line) when the data size exceeds a threshold. Below the threshold, the default pretty JSON formatting is preserved.

Useful for keeping small results readable in a terminal while preventing huge bulk dumps from blowing up the screen with indented multi-line output.

**Requires fimod ≥ 0.6.0** (uses `pipeline.current_step().set('output_format', ...)`).

## Usage

```bash
# Default threshold: 1000 items
fimod s -i events.json -m @compact_if_big

# Custom threshold
fimod s -i events.json -m @compact_if_big --arg max=500
```

## Example

**Small input** (5 rows, default `max=1000`):
```bash
fimod s -i small.json -m @compact_if_big
```

Output stays pretty:
```json
[
  {"id": 1},
  {"id": 2}
]
```

**Big input** (10 000 rows, default `max=1000`):
```bash
fimod s -i big.json -m @compact_if_big
```

Output flips to compact:
```json
[{"id":1},{"id":2}, ... ,{"id":10000}]
```

## Args

| Arg   | Required | Default | Description |
|-------|----------|---------|-------------|
| `max` | No       | `1000`  | Items threshold. `len(data) > max` triggers `output_format = json-compact`. |

## Notes

- **Applies to lists and top-level dicts**: `size = len(data)` for both. Other types (strings, numbers, scalars) leave the format unchanged.
- **Pure side-effect**: returns data unchanged. Acts only on the serialization format of the current step.
- **Last step wins**: if the chain has multiple `set('output_format', ...)` calls, the last one before the end of the pipeline determines the actual serialization. Place `compact_if_big` near the end of the chain to avoid being overridden.

## When to use

- Logs / events / metrics dumps where you want pretty output for small slices but compact for full exports.
- CI pipelines that emit varying-size results — keeps human-readable diffs for the small case, compact bytes for the large case.
- Demonstrates the **adaptive output** pattern of fimod 0.6.0 — see [Dynamic Molds](../../docs/guides/dynamic-molds.md).
