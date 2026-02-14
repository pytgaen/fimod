# sort_json_keys

Recursively sort the keys of a JSON structure. Useful for diffing configs or normalizing output.

## Usage

```bash
fimod s -i config.json -m @sort_json_keys
```

## Example

**Input** (`config.json`):
```json
{"z_feature": true, "a_name": "app", "m_config": {"port": 8080, "host": "localhost"}}
```

**Output**:
```json
{"a_name": "app", "m_config": {"host": "localhost", "port": 8080}, "z_feature": true}
```

Combine with `--in-place` to sort a file in place:

```bash
fimod s -i config.json -m @sort_json_keys --in-place
```
