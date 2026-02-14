"""
Validate that required fields exist (exits 1 if missing).

Usage:
  fimod s -i config.json -m @validate_fields --arg required=database.host,database.port,app.name
"""
# fimod: arg=required  Comma-separated list of dotpaths that must be present (or set FIMOD_REQUIRED_FIELDS env var)

def transform(data, args, env, headers):
    required_arg = args.get("required", "") or env.get("FIMOD_REQUIRED_FIELDS", "")
    if not required_arg:
        return data
    required = [r.strip() for r in required_arg.split(",") if r.strip()]
    missing = []
    for path in required:
        if dp_get(data, path) is None:
            missing.append(path)
    if missing:
        set_exit(1)
        return {"valid": False, "missing": missing}
    return {"valid": True, "missing": []}
