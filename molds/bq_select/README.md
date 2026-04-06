# bq_select

Generate a `SELECT` statement from a BigQuery table.

## Usage

```bash
bq show --format=json project:dataset.table | fimod -m bq_select -O txt
```

## Input

Full JSON from `bq show --format=json` (table name and schema are extracted automatically):

```json
{
  "tableReference": {
    "projectId": "my-project",
    "datasetId": "my_dataset",
    "tableId": "my_table"
  },
  "schema": {
    "fields": [
      {"name": "id", "type": "INTEGER", "mode": "REQUIRED"},
      {"name": "email", "type": "STRING", "mode": "NULLABLE"},
      {"name": "created_at", "type": "TIMESTAMP", "mode": "NULLABLE"}
    ]
  }
}
```

## Output

```sql
SELECT
  id,
  email,
  created_at
FROM
  `my-project.my_dataset.my_table`
```
