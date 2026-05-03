"""
Append @anonymize_pii downstream when the CSV input contains sensitive columns.

Usage:
  fimod s -i users.csv -m @auto_anonymize
  fimod s -i users.csv -m @auto_anonymize --arg detect=email,phone,ssn
"""
# fimod: arg=detect "Comma-separated columns to look for in the header (default: email)"


def transform(data, args, headers, pipeline, **_):
    raw = args.get("detect", "email")
    sensitive = [s.strip() for s in raw.split(",") if s.strip()]
    if not sensitive:
        return data

    found = [s for s in sensitive if headers and s in headers]
    if found:
        pipeline.append(Step.create(
            mold="@anonymize_pii",
            args={"fields": ",".join(found)},
        ))
    return data
