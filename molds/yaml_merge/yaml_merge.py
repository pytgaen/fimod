"""
Merge patch values into an existing YAML structure.

Usage:
  fimod s -i deployment.yaml -m @yaml_merge --arg set="spec.replicas=3,metadata.labels.env=prod"
"""
# fimod: output-format=yaml
# fimod: arg=set  Comma-separated list of path=value assignments

def transform(data, args, env, headers):
    try:
        set_arg = args.get("set", "")
    except NameError:
        set_arg = ""
    if not set_arg:
        return data

    result = data
    for pair in set_arg.split(","):
        parts = pair.split("=", 1)
        if len(parts) == 2:
            path = parts[0].strip()
            val_str = parts[1].strip()

            # Primitive type parsing
            if val_str.lower() == "true":
                val = True
            elif val_str.lower() == "false":
                val = False
            else:
                try:
                    val = int(val_str)
                except ValueError:
                    val = val_str

            result = dp_set(result, path, val)

    return result
