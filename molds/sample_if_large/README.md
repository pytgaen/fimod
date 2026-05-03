# sample_if_large

Inject a sampling step immediately after this one if the list exceeds `max` items.
Passes through unchanged when the list is within the limit or when data is not a list.

**Requires fimod ≥ 0.6.0** (uses the `pipeline` parameter, `Step.create()` constructor, and `msg_info` built-in).

## Usage

```bash
# Keep only the first 500 items before a slow analysis step
fimod s -i logs.json -m @sample_if_large --arg max=500 -m analyze.py

# Keep the last 200 events (most recent) before reporting
fimod s -i events.json -m @sample_if_large --arg max=200 --arg strategy=tail -m report.py
```

## Example

**Input** (`logs.json`, 1000 items):
```json
[1, 2, 3, ..., 1000]
```

```bash
fimod s -i logs.json -m @sample_if_large --arg max=3 -m analyze.py
```

Stderr:
```
[INFO] [step 1/2] sample_if_large: 1000 items → sampling to 3 (head)
```

`analyze.py` receives:
```json
[1, 2, 3]
```

If the list has 3 items or fewer, `analyze.py` receives the full list with no sampling and no log message.

## Args

| Arg | Required | Default | Description |
|-----|----------|---------|-------------|
| `max` | **Yes** | — | Maximum number of items before sampling kicks in |
| `strategy` | No | `head` | `head` keeps the first N items, `tail` keeps the last N |

## Notes

- Non-list data (dicts, strings, numbers) is always passed through unchanged.
- The sampling happens in an injected step, so `pipeline.length()` reflects the updated chain in all subsequent steps.
