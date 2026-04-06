"""Generate an INSERT INTO (VALUES or SELECT) statement from a BigQuery table (bq show --format=json)."""


def transform(data, args, **_):
    """Generate an INSERT INTO statement.

    Input: full JSON from `bq show --format=json project:dataset.table`

    Args:
        exclude: Comma-separated columns to exclude (e.g. auto-generated keys)
        values:  Placeholder style: ?, %s, or @param (default: ?)
        from:    Source table for INSERT ... SELECT (fully qualified name)
    """
    ref = data["tableReference"]
    table = f"{ref['projectId']}.{ref['datasetId']}.{ref['tableId']}"
    values = args.get("values", "?")
    exclude = [e.strip() for e in args.get("exclude", "").split(",") if e.strip()]

    cols = [f["name"] for f in data["schema"]["fields"] if f["name"] not in exclude]
    joined = ", ".join(cols)

    source = args.get("from", "")

    if source:
        select_cols = ",\n  ".join(cols)
        return f"INSERT INTO `{table}` ({joined})\nSELECT\n  {select_cols}\nFROM\n  `{source}`"

    if values == "@param":
        placeholders = ", ".join([f"@{c}" for c in cols])
    else:
        placeholders = ", ".join([values for _ in cols])

    return f"INSERT INTO `{table}` ({joined})\nVALUES ({placeholders})"
