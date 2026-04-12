"""
Keep only specified fields from an object or array of objects.

Usage:
  fimod s -i users.json -m @pick_fields --arg fields=id,name,email
"""
# fimod: arg=fields  "Comma-separated list of fields to keep"

def transform(data, args, **_):
    try:
        fields_arg = args.get("fields", "")
    except NameError:
        fields_arg = ""
    if not fields_arg:
        return data

    fields = [f.strip() for f in fields_arg.split(",") if f.strip()]

    if isinstance(data, list):
        return [{k: v for k, v in d.items() if k in fields} for d in data if isinstance(d, dict)]
    elif isinstance(data, dict):
        return {k: v for k, v in data.items() if k in fields}

    return data
