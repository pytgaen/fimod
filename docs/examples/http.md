# HTTP — Practical Examples

## Simple GET (auto-parsed)

`-i https://...` fetches the URL and auto-detects the format from `Content-Type`. No flag needed for JSON APIs:

```bash
fimod s -i https://jsonplaceholder.typicode.com/users \
  -e '[{"name": u["name"], "city": dp_get(u, "address.city")} for u in data]'
```
```json
[
  {"name": "Leanne Graham", "city": "Gwenborough"},
  ...
]
```

---

## Inspect status and headers (`--input-format http`)

Use `--input-format http` to access the full response envelope:

```bash
fimod s -i https://jsonplaceholder.typicode.com/users \
  --input-format http \
  -e '{"status": data["status"], "content_type": data["headers"]["content-type"]}'
```
```json
{"status": 200, "content_type": "application/json; charset=utf-8"}
```

---

## Inspect a redirect target (`--no-follow`)

```bash
fimod s -i https://github.com/pytgaen/fimod/releases/latest \
  --input-format http --no-follow \
  -e 'data["headers"]["location"].split("/")[-1]' \
  --output-format txt
# → v0.3.0
```

---

## Re-parse the body with `set_input_format()`

With `--input-format http`, `data["body"]` is a raw string. Chain a second `-e` after re-parsing:

```bash
fimod s -i https://api.github.com/repos/pytgaen/fimod/releases/latest \
  --input-format http \
  -e 'set_input_format("json"); data["body"]' \
  -e '{"tag": data["tag_name"], "date": data["published_at"]}'
```

---

## Custom request headers

```bash
# Authenticated API call (requires $GITHUB_TOKEN)
fimod s -i https://api.github.com/user \
  --http-header "Authorization: Bearer $GITHUB_TOKEN" \
  -e '{"login": data["login"], "repos": data["public_repos"]}'
```

Multiple headers — use `https://httpbin.org/headers` to inspect what is sent:

```bash
fimod s -i https://httpbin.org/headers \
  --http-header "X-Custom: hello" \
  --http-header "Accept: application/json" \
  -e 'data["headers"]'
```

---

## Download a binary file

```bash
fimod s -i https://httpbin.org/bytes/1024 \
  --input-format http \
  -e 'set_format("raw"); set_output_file("/tmp/sample.bin"); data'
```

---

## HTTP in a mold script

Validate the status before processing the body:

```bash
cat > /tmp/check_api.py << 'EOF'
def transform(data, args, env, headers):
    if data["status"] != 200:
        gk_fail(f"API returned {data['status']}")
    set_input_format("json")
    return data["body"]
EOF

fimod s -i https://jsonplaceholder.typicode.com/todos/1 \
  --input-format http \
  -m /tmp/check_api.py \
  -e '{"id": data["id"], "done": data["completed"]}'
```
```json
{"id": 1, "done": false}
```
