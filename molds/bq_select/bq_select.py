"""Generate a SELECT statement from a BigQuery table (bq show --format=json)."""


def transform(data, **_):
    """Generate a SELECT statement.

    Input: full JSON from `bq show --format=json project:dataset.table`
    """
    ref = data["tableReference"]
    table = f"{ref['projectId']}.{ref['datasetId']}.{ref['tableId']}"
    cols = [f["name"] for f in data["schema"]["fields"]]
    joined = ",\n  ".join(cols)
    return f"SELECT\n  {joined}\nFROM\n  `{table}`"
