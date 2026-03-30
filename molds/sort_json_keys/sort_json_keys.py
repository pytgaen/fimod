"""
Recursively sort the keys of a JSON structure.

Usage:
  fimod s -i config.json -m @sort_json_keys
"""

def sort_keys_recursive(d):
    if isinstance(d, dict):
        # Convert items to list of tuples and sort them by key
        items = list(d.items())
        items.sort(key=lambda x: x[0])
        return {k: sort_keys_recursive(v) for k, v in items}
    elif isinstance(d, list):
        return [sort_keys_recursive(x) for x in d]
    else:
        return d

def transform(data, **_):
    return sort_keys_recursive(data)
