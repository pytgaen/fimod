"""
Switch the pipeline output to compact JSON when the data exceeds a given size threshold. Default threshold is 1000 items; override with --arg max=N.

Usage:
  fimod s -i logs.json -m @compact_if_big
  fimod s -i logs.json -m @compact_if_big --arg max=500
"""
# fimod: arg=max "Threshold above which output_format switches to json-compact (default: 1000)"


def transform(data, args, pipeline, **_):
    raw_max = args.get("max", "1000")
    try:
        max_items = int(raw_max)
    except (TypeError, ValueError):
        raise ValueError(
            f"compact_if_big: --arg max must be an integer, got '{raw_max}'"
        )
    if max_items <= 0:
        raise ValueError(
            f"compact_if_big: --arg max must be > 0, got {max_items}"
        )

    size = None
    if isinstance(data, list):
        size = len(data)
    elif isinstance(data, dict):
        size = len(data)

    if size is not None and size > max_items:
        pipeline.current_step().set('output_format', 'json-compact')

    return data
