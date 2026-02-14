"""
Flatten a nested JSON object into dot-path keys.

Usage:
  fimod s -i config.json -m @flatten_nested
"""

def _flatten(d, result, parent_key='', sep='.'):
    for k, v in d.items():
        new_key = f"{parent_key}{sep}{k}" if parent_key else str(k)
        if isinstance(v, dict) and v:
            _flatten(v, result, new_key, sep=sep)
        elif isinstance(v, list) and v:
            for i, item in enumerate(v):
                _flatten({str(i): item}, result, new_key, sep=sep)
        else:
            # Preserves empty dicts {}, empty lists [], None, and primitives
            result[new_key] = v

def transform(data, args, env, headers):
    if not isinstance(data, dict):
        return data
    result = {}
    _flatten(data, result)
    return result
