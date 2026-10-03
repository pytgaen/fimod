"""
Deduplicate records by a field (keeps first occurrence).

Usage:
  fimod s -i events.json -m @dedup_by --arg field=event_id
"""
# fimod: arg=field  Field name to deduplicate on

import json


def unique_by(rows, field):
    seen = set()
    result = []
    for row in rows:
        value = row.get(field) if isinstance(row, dict) else None
        key = json.dumps(value)
        if key not in seen:
            seen.add(key)
            result.append(row)
    return result


def transform(data, args, **_):
    try:
        field = args["field"]
    except KeyError:
        return data
    return unique_by(data, field)
