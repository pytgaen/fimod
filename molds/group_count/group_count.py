"""
Group records by field and count occurrences.

Usage:
  fimod s -i logs.json -m @group_count --arg field=status
"""
# fimod: arg=field  Field name to group by

def transform(data, args, **_):
    try:
        field = args["field"]
    except KeyError:
        return data
    grouped = it_group_by(data, field)
    result = []
    for key in it_keys(grouped):
        result.append({"value": key, "count": len(grouped[key])})
    return it_sort_by(result, "count")
