# with_threshold

Compute a percentile threshold from a numeric column and inject a downstream filter step that keeps rows on the requested side of that threshold. Lets you filter outliers from live statistics — no need to know the cut-off value in advance.

**Requires fimod ≥ 0.6.0** (uses `pipeline.insert_next(Step.create(args=...))` — heterogeneous types in `args`).

## Usage

```bash
# Keep events slower than the 95th percentile (default)
fimod s -i events.json -m @with_threshold --arg col=duration_ms

# Keep the top 1% slowest events
fimod s -i events.json -m @with_threshold --arg col=duration_ms --arg pct=99

# Keep the bottom 10% scores
fimod s -i scores.json -m @with_threshold --arg col=score --arg pct=10 --arg op="<"
```

## Example

**Input** (`events.json`):
```json
[
  {"id": 1, "duration_ms": 12},
  {"id": 2, "duration_ms": 45},
  {"id": 3, "duration_ms": 87},
  {"id": 4, "duration_ms": 230},
  {"id": 5, "duration_ms": 1500}
]
```

```bash
fimod s -i events.json -m @with_threshold --arg col=duration_ms --arg pct=80
```

Stderr:
```
[INFO] [step 1/2] with_threshold: p80.0 of 'duration_ms' = 484 (5 values)
```

Output (only rows with `duration_ms > 484`):
```json
[
  {"id": 5, "duration_ms": 1500}
]
```

## Args

| Arg   | Required | Default | Description |
|-------|----------|---------|-------------|
| `col` | **Yes**  | —       | Numeric column name to compute the threshold on |
| `pct` | No       | `95`    | Percentile in 1–99 (interpolated linearly between ranks) |
| `op`  | No       | `>`     | Comparison operator. Allowed: `>`, `>=`, `<`, `<=`, `==`, `!=` |

## Notes

- **Non-list inputs**: dicts, strings, numbers, and empty lists are passed through unchanged. The mold only acts on a non-empty list of dicts.
- **Numeric type coercion**: only `int` and `float` values are considered for the percentile. Booleans are explicitly excluded (Python's `True`/`False` would otherwise count as `1`/`0`). Rows where the column is missing or non-numeric are ignored when computing the threshold and dropped by the downstream filter.
- **Percentile method**: linear interpolation between the two closest ranks (NIST primary, type 7 in R). For a list of N values sorted ascending, `pct=p` returns `v[lo]*(1-frac) + v[hi]*frac` where `k = (N-1)*p/100`, `lo = floor(k)`, `frac = k-lo`.
- **Injected step**: the filter is materialised as `pipeline.insert_next(Step.create(expr=..., args={"threshold": ...}))`. The threshold is propagated as a typed float via `args` (P1 — heterogeneous arg types).
- **Security**: the column name is interpolated into the filter expression via `repr()` (escapes quotes); the operator is strictly validated against a whitelist before interpolation. No code injection possible from `--arg` values.

## When to use

- Cutting off slow API calls / DB queries above a live percentile.
- Keeping only the lowest scores / highest priorities for triage.
- Any workflow where the threshold is data-dependent and not known a priori.
