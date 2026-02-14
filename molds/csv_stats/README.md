# csv_stats

Compute basic statistics (min, max, mean, count) for numeric columns in a CSV file. Non-numeric columns are ignored.

## Usage

```bash
fimod s -i sales.csv -m @csv_stats
```

## Example

**Input** (`sales.csv`):
```csv
product,price,quantity
Widget,9.99,100
Gadget,24.50,42
Gizmo,5.00,200
```

**Output**:
```json
{
  "price": {"count": 3, "min": 5.0, "max": 24.5, "mean": 13.163},
  "quantity": {"count": 3, "min": 42.0, "max": 200.0, "mean": 114.0}
}
```

The `product` column is automatically skipped because it contains non-numeric values.
