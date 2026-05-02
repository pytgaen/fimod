"""
Inject a sampling step immediately after this one if the list exceeds `max` items.
Passes through unchanged when the list is within the limit or when data is not a list.

Usage:
  fimod s -i logs.json -m @sample_if_large --arg max=500 -m analyze.py
  fimod s -i events.json -m @sample_if_large --arg max=200 --arg strategy=tail -m report.py
"""
# fimod: arg=max      "Maximum number of items before sampling kicks in (required)"
# fimod: arg=strategy "head keeps the first N items (default), tail keeps the last N"


def transform(data, args, pipeline, **_):
    raw_max = args.get("max")
    if raw_max is None or raw_max == "":
        raise ValueError("sample_if_large: --arg max=N is required (positive integer)")
    try:
        max_items = int(raw_max)
    except (TypeError, ValueError):
        raise ValueError(f"sample_if_large: --arg max must be an integer, got '{raw_max}'")
    if max_items <= 0:
        raise ValueError(f"sample_if_large: --arg max must be > 0, got {max_items}")

    if not isinstance(data, list):
        return data

    if len(data) <= max_items:
        return data

    strategy = args.get("strategy", "head")
    step = pipeline.current_step()

    msg_info(
        f"[step {step.get('index') + 1}/{pipeline.length()}] "
        f"sample_if_large: {len(data)} items → sampling to {max_items} ({strategy})"
    )

    if strategy == "tail":
        pipeline.insert_next(Step.create(expr=f"data[-{max_items}:]"))
    else:
        pipeline.insert_next(Step.create(expr=f"data[:{max_items}]"))

    return data
