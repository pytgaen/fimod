# csv_to_json_records

Convert a CSV file into a JSON array of objects, using the header row as keys.

## Usage

```bash
fimod s -i users.csv -m @csv_to_json_records
```

## Example

**Input** (`users.csv`):
```csv
name,email,age
Alice,alice@example.com,30
Bob,bob@example.com,25
```

**Output**:
```json
[
  {"name": "Alice", "email": "alice@example.com", "age": "30"},
  {"name": "Bob", "email": "bob@example.com", "age": "25"}
]
```

*Note: This mold is a convenience shortcut that guarantees `input-format=csv` and `output-format=json`. Fimod already parses CSV with headers into a list of dicts.*
