"""
Extract a simplified JSON schema from a document.

Usage:
  fimod s -i sample.json -m @json_schema_extract
"""

def extract_type(val):
    if isinstance(val, dict):
        return {k: extract_type(v) for k, v in val.items()}
    elif isinstance(val, list):
        return "list"
    elif isinstance(val, bool):
        return "boolean"
    elif isinstance(val, int):
        return "integer"
    elif isinstance(val, float):
        return "number"
    elif isinstance(val, str):
        return "string"
    elif val is None:
        return "null"
    else:
        return "any"

def transform(data, **_):
    return extract_type(data)
