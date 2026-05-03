"""
Compute a percentile threshold from a numeric column and inject a downstream filter step keeping rows on the requested side of that threshold.

Usage:
  fimod s -i events.json -m @with_threshold --arg col=duration_ms
  fimod s -i events.json -m @with_threshold --arg col=duration_ms --arg pct=99
  fimod s -i scores.json  -m @with_threshold --arg col=score --arg pct=10 --arg op="<"
"""
# fimod: arg=col "Numeric column name to compute the threshold on (required)"
# fimod: arg=pct "Percentile in 1-99 (default: 95)"
# fimod: arg=op  "Comparison operator: >, >=, <, <=, ==, != (default: >)"


_ALLOWED_OPS = (">", ">=", "<", "<=", "==", "!=")


def _percentile(sorted_values, pct):
    n = len(sorted_values)
    if n == 1:
        return sorted_values[0]
    k = (n - 1) * pct / 100.0
    lo = int(k)
    hi = lo + 1
    frac = k - lo
    if hi >= n:
        return sorted_values[-1]
    return sorted_values[lo] * (1 - frac) + sorted_values[hi] * frac


def transform(data, args, pipeline, **_):
    col = args.get("col")
    if not col:
        raise ValueError("with_threshold: --arg col=NAME is required")

    raw_pct = args.get("pct", "95")
    try:
        pct = float(raw_pct)
    except (TypeError, ValueError):
        raise ValueError(
            f"with_threshold: --arg pct must be a number, got '{raw_pct}'"
        )
    if not (0 < pct < 100):
        raise ValueError(
            f"with_threshold: --arg pct must be between 1 and 99, got {pct}"
        )

    op = args.get("op", ">")
    if op not in _ALLOWED_OPS:
        raise ValueError(
            f"with_threshold: --arg op must be one of {list(_ALLOWED_OPS)}, got '{op}'"
        )

    if not isinstance(data, list) or not data:
        return data

    values = []
    for row in data:
        if not isinstance(row, dict):
            continue
        v = row.get(col)
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            values.append(v)

    if not values:
        raise ValueError(
            f"with_threshold: column '{col}' has no numeric values"
        )

    values.sort()
    threshold = _percentile(values, pct)

    step = pipeline.current_step()
    msg_info(
        f"[step {step.get('index') + 1}/{pipeline.length()}] "
        f"with_threshold: p{pct} of '{col}' = {threshold} ({len(values)} values)"
    )

    expr = (
        f"[r for r in data if isinstance(r, dict) "
        f"and isinstance(r.get({col!r}), (int, float)) "
        f"and not isinstance(r.get({col!r}), bool) "
        f"and r[{col!r}] {op} args['threshold']]"
    )
    pipeline.insert_next(Step.create(expr=expr, args={"threshold": threshold}))

    return data
