"""
Split a delimiter-separated tags field into a list and deduplicate.

Usage:
  fimod s -i articles.json -m @split_tags --arg field=tags
"""
# fimod: arg=field    Field name containing the tags string
# fimod: arg=sep      "Separator regex (default: comma/semicolon with optional space)"

import re


def split_with_captures(pattern, text):
    # Monty 1.0.0 re.split omits captured separators; preserve this mold's API.
    parts = []
    last_end = 0
    for match in re.finditer(pattern, text):
        parts.append(text[last_end:match.start()])
        parts.extend(match.groups())
        last_end = match.end()
    parts.append(text[last_end:])
    return parts


def transform(data, args, **_):
    try:
        field = args["field"]
    except KeyError:
        return data
    sep = args.get("sep", r"[,;]\s*")

    def split_one(obj):
        if isinstance(obj, dict) and field in obj:
            raw = obj[field]
            if isinstance(raw, str):
                parts = split_with_captures(sep, raw)
                obj[field] = list(dict.fromkeys(p.strip() for p in parts if p.strip()))
        return obj

    if isinstance(data, list):
        return [split_one(row) for row in data]
    elif isinstance(data, dict):
        return split_one(data)
    return data
