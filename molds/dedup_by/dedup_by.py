"""
Deduplicate records by a field (keeps first occurrence).

Usage:
  fimod s -i events.json -m @dedup_by --arg field=event_id
"""
# fimod: arg=field  Field name to deduplicate on

def transform(data, args, **_):
    try:
        field = args["field"]
    except KeyError:
        return data
    return it_unique_by(data, field)
