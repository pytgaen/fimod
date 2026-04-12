"""
Hash specified fields with SHA-256 for anonymization.

Usage:
  fimod s -i users.json -m @anonymize_pii --arg fields=email,phone
"""
# fimod: arg=fields  "Comma-separated list of fields to anonymize"

def transform(data, args, **_):
    try:
        fields_arg = args["fields"]
    except KeyError:
        return data
    fields = [f.strip() for f in fields_arg.split(",") if f.strip()]
    if isinstance(data, list):
        for row in data:
            if isinstance(row, dict):
                for f in fields:
                    if f in row:
                        row[f] = hs_sha256(str(row[f]))
        return data
    elif isinstance(data, dict):
        for f in fields:
            if f in data:
                data[f] = hs_sha256(str(data[f]))
        return data
    return data
