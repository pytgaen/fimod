"""
Convert Skylos dead code reports to GitLab Code Quality format.

Usage:
  fimod s -i skylos_report.json -m @skylos_to_gitlab -o code_quality.json

In a GitLab CI pipeline:
  skylos --json > skylos_report.json
  fimod s -i skylos_report.json -m @skylos_to_gitlab -o gl-code-quality-report.json
"""
# fimod: output-format=json

def transform(data, args, env, headers):
    """
    Convert duriantaco/skylos JSON report to GitLab Code Quality (Code Climate) format.

    Skylos Output Structure (analyzed):
    {
      "unused_functions": [ {"name": "foo", "file": "...", "line": 10}, ... ],
      "unused_imports": [...],
      "unused_variables": [...],
      ...
    }
    """

    issues = []

    # Map Skylos sections to readable check names
    categories = {
        "unused_functions": "unused-function",
        "unused_imports":   "unused-import",
        "unused_variables": "unused-variable",
        "unused_classes":   "unused-class",
        "unused_parameters":"unused-parameter",
        "unused_files":     "dead-file"
    }

    for key, check_name in categories.items():
        items = data.get(key, [])
        if not isinstance(items, list):
            continue

        for item in items:
            # Extract info
            name = item.get("name") or item.get("simple_name") or "unknown"
            path = item.get("file") or item.get("filename") or "unknown"
            line = item.get("line") or 1

            # Construct message
            # e.g. "Unused function: transform"
            readable_type = key.replace("unused_", "").replace("s", "") # function, import...
            description = f"Unused {readable_type}: {name}"

            # Generate stable fingerprint
            fingerprint_raw = f"{check_name}:{path}:{line}:{name}"
            fingerprint = hs_md5(fingerprint_raw)

            issues.append({
                "description": description,
                "check_name": check_name,
                "fingerprint": fingerprint,
                "severity": "info", # unused code is usually info/minor
                "location": {
                    "path": path,
                    "lines": {
                        "begin": int(line)
                    }
                }
            })

    return issues
