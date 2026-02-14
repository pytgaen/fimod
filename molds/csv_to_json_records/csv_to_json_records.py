"""
Convert CSV to JSON array of objects.

Usage:
  fimod s -i users.csv -m @csv_to_json_records
"""
# fimod: output-format=json, input-format=csv

# Simple passthrough since Fimod already parses CSV with headers into a list of dictionaries.
def transform(data, args, env, headers):
    if not isinstance(data, list):
        return []
    return [row for row in data]
