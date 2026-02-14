"""
Common jq operations (get, map, select) via --arg.

Usage:
  fimod s -i data.json -m @jq_compat --arg get=user.address.city
  fimod s -i users.json -m @jq_compat --arg map=name
  fimod s -i users.json -m @jq_compat --arg select=active=true
"""

def transform(data, args, env, headers):
    if not args:
        return data

    if "get" in args:
        return dp_get(data, args["get"])

    # --arg map "field"
    if "map" in args and isinstance(data, list):
        field = args["map"]
        return [d.get(field) for d in data if isinstance(d, dict) and field in d]

    # --arg select "field=value"
    if "select" in args and isinstance(data, list):
        parts = args["select"].split("=", 1)
        if len(parts) == 2:
            k, v = parts[0].strip(), parts[1].strip()
            return [d for d in data if isinstance(d, dict) and str(d.get(k, "")) == v]

    return data
