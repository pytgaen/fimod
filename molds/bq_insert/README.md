# bq_insert

Generate an `INSERT INTO` statement from a BigQuery table.

## Usage

```bash
bq show --format=json project:dataset.table | fimod -m bq_insert -O txt
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
INSERT INTO `my-project.my_dataset.my_table` (id, email, created_at)
VALUES (?, ?, ?)
```

## Args

| Arg       | Default  | Description                                      |
|-----------|----------|--------------------------------------------------|
| `exclude` | *(none)* | Comma-separated columns to exclude (e.g. auto-generated keys) |
| `values`  | `?`      | Placeholder style: `?`, `%s`, or `@param`        |
| `from`    | *(none)* | Source table for `INSERT ... SELECT` form         |

## Examples

```bash
# Exclude id, use BigQuery named params
bq show --format=json project:dataset.table \
  | fimod -m bq_insert --arg exclude=id --arg values=@param -O txt
```

```sql
INSERT INTO `my-project.my_dataset.my_table` (email, created_at)
VALUES (@email, @created_at)
```

```bash
# INSERT ... SELECT from another table
bq show --format=json project:dataset.table \
  | fimod -m bq_insert --arg from=other-project.other_dataset.source_table -O txt
```

```sql
INSERT INTO `my-project.my_dataset.my_table` (id, email, created_at)
SELECT
  id,
  email,
  created_at
FROM
  `other-project.other_dataset.source_table`
```

```bash
# INSERT ... SELECT, excluding auto-generated id
bq show --format=json project:dataset.table \
  | fimod -m bq_insert --arg from=other-project.other_dataset.source_table --arg exclude=id -O txt
```

```sql
INSERT INTO `my-project.my_dataset.my_table` (email, created_at)
SELECT
  email,
  created_at
FROM
  `other-project.other_dataset.source_table`
```
