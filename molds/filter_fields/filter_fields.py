"""
Filter fields of an object or array of objects by keeping or dropping dotpaths.

Supports nested paths (e.g. "user.email", "meta.tags.0") via dp_has/dp_get/dp_set/dp_delete.

Usage:
  fimod s -i users.json -m @filter_fields --arg mode=drop --arg fields=password,meta.debug
  fimod s -i users.json -m @filter_fields --arg mode=keep --arg fields=id,user.email
"""
# fimod: arg=mode    "keep or drop — whether to retain or remove the listed paths (default: keep)"
# fimod: arg=fields  "Comma-separated list of dotpaths"

def transform(data, args, **_):
    mode = args.get("mode", "keep").strip().lower()
    fields_arg = args.get("fields", "").strip()

    if mode not in ("keep", "drop"):
        gk_fail(f"filter_fields: mode must be 'keep' or 'drop', got {mode!r}")
        return data
    if not fields_arg:
        return data

    paths = [p.strip() for p in fields_arg.split(",") if p.strip()]

    def apply(obj):
        if not isinstance(obj, dict):
            return obj
        if mode == "drop":
            result = obj
            for path in paths:
                result = dp_delete(result, path)
            return result
        result = {}
        for path in paths:
            if dp_has(obj, path):
                result = dp_set(result, path, dp_get(obj, path))
        return result

    if isinstance(data, list):
        return [apply(d) for d in data]
    return apply(data)
