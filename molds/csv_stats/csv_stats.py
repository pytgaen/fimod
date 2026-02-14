"""
Compute basic statistics on numeric CSV columns.

Usage:
  fimod s -i sales.csv -m @csv_stats
"""
# fimod: input-format=csv
# fimod: output-format=json

def transform(data, args, env, headers):
    if not isinstance(data, list) or len(data) == 0:
        return {}

    # data is a list of dicts (csv with headers)
    if not isinstance(data[0], dict):
        return {"error": "Expected CSV to have headers"}

    columns = data[0].keys()
    stats = {}

    for col in columns:
        vals = []
        for row in data:
            v_str = row.get(col, "")
            if isinstance(v_str, str):
                v_str = v_str.strip()

            # Simple float parser
            try:
                 vals.append(float(v_str))
            except ValueError:
                 pass

        if vals:
            count = len(vals)
            stats[col] = {
                "count": count,
                "max": max(vals),
                "mean": sum(vals) / count,
                "min": min(vals)
            }

    return stats
