"""
Rename keys via mapping 'old1:new1,old2:new2'.

Usage:
  fimod s -i data.json -m @rename_keys --arg mapping=firstName:first_name,lastName:last_name
"""
# fimod: arg=mapping  Comma-separated list of old_key:new_key pairs

def transform(data, args, **_):
    try:
        mapping_arg = args.get("mapping", "")
    except NameError:
        mapping_arg = ""
    if not mapping_arg:
        return data

    mapping = {}
    for pair in mapping_arg.split(","):
        parts = pair.split(":")
        if len(parts) == 2:
            mapping[parts[0].strip()] = parts[1].strip()

    def rename_dict(d):
        return {mapping.get(k, k): v for k, v in d.items()}

    if isinstance(data, list):
        return [rename_dict(d) if isinstance(d, dict) else d for d in data]
    elif isinstance(data, dict):
        return rename_dict(data)

    return data
