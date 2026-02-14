# split_tags

Split a delimiter-separated tags field into a list and deduplicate. Default separator matches commas and semicolons.

## Usage

```bash
fimod s -i articles.json -m @split_tags --arg field=tags
```

## Example

**Input** (`articles.json`):
```json
[
  {"title": "Post 1", "tags": "rust, python, rust"},
  {"title": "Post 2", "tags": "go; docker; go"}
]
```

**Output**:
```json
[
  {"title": "Post 1", "tags": ["rust", "python"]},
  {"title": "Post 2", "tags": ["go", "docker"]}
]
```

### Custom separator

```bash
fimod s -i data.json -m @split_tags --arg field=categories --arg sep="|"
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `field` | Yes | Field name containing the tags string |
| `sep` | No | Separator regex (default: `[,;]\s*` — comma or semicolon with optional space) |
