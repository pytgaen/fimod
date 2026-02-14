# JSON — Practical Examples

## Input: object, array, scalar

`data` is whatever the root JSON value is — object, array, or scalar:

```bash
echo '{"name":"Alice","age":30}' | fimod s -e 'data["name"]' --output-format txt
# → Alice

echo '[1,2,3]' | fimod s -e 'len(data)' --output-format txt
# → 3

echo '"hello"' | fimod s -e 'data.upper()' --output-format txt
# → HELLO
```

---

## Filter an array

```bash
echo '[{"name":"Alice","active":true},{"name":"Bob","active":false}]' | \
  fimod s -e '[x for x in data if x["active"]]'
```
```json
[
  {
    "name": "Alice",
    "active": true
  }
]
```

---

## Pick / reshape keys

```bash
echo '[{"id":1,"name":"Alice","email":"a@x.com","role":"admin"},{"id":2,"name":"Bob","email":"b@x.com","role":"user"}]' | \
  fimod s -e '[{"id": x["id"], "name": x["name"]} for x in data]'
```
```json
[
  {"id": 1, "name": "Alice"},
  {"id": 2, "name": "Bob"}
]
```

---

## Rename keys

```bash
echo '[{"FirstName":"Alice","LastName":"Smith"},{"FirstName":"Bob","LastName":"Jones"}]' | \
  fimod s -e '[{"first_name": x["FirstName"], "last_name": x["LastName"]} for x in data]'
```
```json
[
  {"first_name": "Alice", "last_name": "Smith"},
  {"first_name": "Bob", "last_name": "Jones"}
]
```

---

## Pretty-print / compact

```bash
# Pretty-print (replaces `jq .`)
echo '{"name":"Alice","address":{"city":"Paris","zip":"75001"}}' | fimod s -e 'data'

# Compact output
echo '{"name":"Alice","address":{"city":"Paris","zip":"75001"}}' | fimod s -e 'data' --compact
```

For a file:

```bash
cat > /tmp/data.json << 'EOF'
{"name":"Alice","address":{"city":"Paris","zip":"75001"}}
EOF
fimod s -i /tmp/data.json -e 'data'
fimod s -i /tmp/data.json -e 'data' --in-place
```

---

## Convert to other formats

```bash
echo '{"host":"localhost","port":5432,"db":"myapp"}' | fimod s -e 'data' --output-format yaml
echo '{"host":"localhost","port":5432,"db":"myapp"}' | fimod s -e 'data' --output-format toml

# JSON array → NDJSON
echo '[{"id":1},{"id":2},{"id":3}]' | fimod s -e 'data' --output-format ndjson
```

---

## NDJSON

```bash
# Filter an NDJSON stream
printf '{"level":"info","msg":"start"}\n{"level":"error","msg":"fail"}\n{"level":"info","msg":"end"}\n' | \
  fimod s --input-format ndjson -e '[e for e in data if e["level"] == "error"]'
```
```json
[
  {"level": "error", "msg": "fail"}
]
```

```bash
# NDJSON → JSON array
printf '{"id":1}\n{"id":2}\n{"id":3}\n' | \
  fimod s --input-format ndjson -e 'data' --output-format json
```

!!! info "NDJSON vs Lines"
    `ndjson` parses each line as JSON. `lines` treats each line as a raw string.
