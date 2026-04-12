"""
Extract nested fields by dotpath into a flat object.

Usage:
  fimod s -i users.json -m @deep_pluck --arg paths=user.name,user.address.city
"""
# fimod: arg=paths  "Comma-separated dotpaths to extract (e.g. user.name,user.email)"

def transform(data, args, **_):
    try:
        paths_arg = args["paths"]
    except KeyError:
        return data
    paths = [p.strip() for p in paths_arg.split(",") if p.strip()]

    def pluck_one(obj):
        result = {}
        for path in paths:
            # Use the last segment as the output key
            key = path.split(".")[-1]
            result[key] = dp_get(obj, path)
        return result

    if isinstance(data, list):
        return [pluck_one(item) for item in data if isinstance(item, dict)]
    elif isinstance(data, dict):
        return pluck_one(data)
    return data
