"""
Split a delimiter-separated tags field into a list and deduplicate.

Usage:
  fimod s -i articles.json -m @split_tags --arg field=tags
"""
# fimod: arg=field    Field name containing the tags string
# fimod: arg=sep      Separator regex (default: comma/semicolon with optional space)

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
                parts = re_split(sep, raw)
                obj[field] = it_unique([p.strip() for p in parts if p.strip()])
        return obj

    if isinstance(data, list):
        return [split_one(row) for row in data]
    elif isinstance(data, dict):
        return split_one(data)
    return data
