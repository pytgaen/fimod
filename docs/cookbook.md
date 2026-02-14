# 🍳 Cookbook

Practical examples of common data transformation tasks using fimod. All examples are compatible with Monty's Python subset.

## 🔄 Basic Transformations

### 🏷️ Renaming Keys

```python
def transform(data, args, env, headers):
    for row in data:
        if "First Name" in row:
            row["first_name"] = row["First Name"]
        if "Age" in row:
            row["age"] = int(row["Age"])
    return data
```

## 🏗️ Data Structuring

### 📊 Flat CSV to Nested JSON

Convert flat rows into a structured object indexed by ID.

**Input (CSV):**
```csv
id,role,permission
101,admin,read
101,admin,write
102,user,read
```

**Script:**
```python
def transform(data, args, env, headers):
    result = {}
    for row in data:
        user_id = row["id"]
        if user_id not in result:
            result[user_id] = {"role": row["role"], "permissions": []}
        result[user_id]["permissions"].append(row["permission"])
    return result
```

**Output (JSON):**
```json
{
  "101": {"role": "admin", "permissions": ["read", "write"]},
  "102": {"role": "user", "permissions": ["read"]}
}
```

## 🧹 Data Cleaning

### 🔒 Masking Sensitive Data

```python
def transform(data, args, env, headers):
    for user in data:
        if "email" in user:
            parts = user["email"].split("@")
            user["email"] = f"{parts[0][0]}***@{parts[1]}"
    return data
```

### 🧽 Deduplication + Normalization

```python
def transform(data, args, env, headers):
    seen = {}
    result = []
    for row in data:
        email = row["email"].strip().lower()
        if email in seen:
            continue
        seen[email] = True
        result.append({
            "name": f"{row['first_name'].strip().title()} {row['last_name'].strip().title()}",
            "email": email,
            "department": (row.get("dept") or "unknown").upper(),
        })
    return result
```

## 📊 Aggregation

### 📈 Group by + Average

```python
def transform(data, args, env, headers):
    depts = {}
    for e in data:
        d = e["department"]
        if d not in depts:
            depts[d] = {"dept": d, "count": 0, "total": 0}
        entry = depts[d]
        entry["count"] = entry["count"] + 1
        entry["total"] = entry["total"] + e["salary"]
    result = []
    for entry in depts.values():
        result.append({
            "dept": entry["dept"],
            "count": entry["count"],
            "avg_salary": entry["total"] / entry["count"]
        })
    return result
```

## 🔍 Regex Recipes

### 📧 Extract Email Addresses

```python
def transform(data, args, env, headers):
    return {"emails": re_findall(r"\w+@\w+\.\w+", data["text"])}
```

### 🧽 Normalize Whitespace

```python
def transform(data, args, env, headers):
    return {"cleaned": re_sub(r"\s+", " ", data["text"].strip())}
```

### 🔗 Extract URLs

```python
def transform(data, args, env, headers):
    urls = re_findall(r"https?://[^\s]+", data["text"])
    return {"urls": urls, "count": len(urls)}
```

### 🏷️ Parse Structured Strings

```python
# Parse "KEY=VALUE" pairs from config text
def transform(data, args, env, headers):
    pairs = re_findall(r"(\w+)=(\S+)", data["text"])
    # With 2 capture groups, re_findall returns [["key","val"], ...]
    result = {}
    for pair in pairs:
        result[pair[0]] = pair[1]
    return result
```

Or with named groups:

```python
def transform(data, args, env, headers):
    result = {}
    for line in data["text"].strip().split("\n"):
        m = re_search(r"^(?P<key>\w+)=(?P<val>.+)$", line)
        if m:
            result[m["named"]["key"]] = m["named"]["val"]
    return result
```

### 🔐 Validate Patterns

```python
# Check if values match expected patterns
def transform(data, args, env, headers):
    for row in data:
        phone = row.get("phone", "")
        row["valid_phone"] = re_match(r"\+?\d{10,15}", phone) is not None
    return data
```

## 📃 Log Analysis

### 🔎 Filter Error Lines

```bash
fimod s -i app.log --input-format lines \
  -e '[l for l in data if "ERROR" in l]'
```

### 📊 Count by Level

```python
def transform(data, args, env, headers):
    levels = {}
    for line in data:
        for level in ["ERROR", "WARN", "INFO", "DEBUG"]:
            if level in line:
                levels[level] = levels.get(level, 0) + 1
    return levels
```

### 🔍 Filter with Regex

```bash
# Lines with 4xx/5xx status codes
fimod s -i access.log --input-format lines \
  -e '[l for l in data if re_search(r"\s[45]\d{2}\s", l)]'
```

## 🌐 API & HTTP

Awesome: the input can be an HTTPS request! Just `-i https://...` and you're done.

### 📉 Nested API → Flat CSV

Fetch, transform, and save as CSV in one command — using [JSONPlaceholder](https://jsonplaceholder.typicode.com) as a live public API:

```bash
fimod s -i https://jsonplaceholder.typicode.com/users \
  -e '[{
    "id": u["id"],
    "name": u["name"],
    "email": u["email"],
    "city": dp_get(u, "address.city"),
    "company": dp_get(u, "company.name")
  } for u in data]' \
  -o contacts.csv
```

`dp_get` safely navigates nested fields (`address.city`, `company.name`) without risking a `KeyError`.

### 🔍 Get the Latest Release Tag

```bash
# Inspect the redirect to extract the version
fimod s -i https://github.com/pytgaen/fimod/releases/latest \
    --input-format http --no-follow \
    -e 'data["headers"]["location"].split("/")[-1]' --output-format txt
# → v0.3.0
```

### 🔁 Fetch + Re-parse with `set_input_format()`

```bash
# Get raw HTTP response, then parse the body as JSON
fimod s -i https://api.github.com/repos/pytgaen/fimod/releases/latest \
    --input-format http \
    -e 'set_input_format("json"); data["body"]' \
    -e '{"tag": data["tag_name"], "date": data["published_at"]}'
```

## 📎 Parameterized Scripts (`--arg`)

### 🎯 Reusable Filter

```python
# filter_by_field.py — generic filter script
def transform(data, args, env, headers):
    field = args["field"]
    value = args["value"]
    return [row for row in data if row.get(field) == value]
```

```bash
# Reuse with different parameters
fimod s -i users.json -m filter_by_field.py --arg field="role" --arg value="admin"
fimod s -i users.json -m filter_by_field.py --arg field="status" --arg value="active"
```

### 🔢 Threshold with Type Casting

```python
def transform(data, args, env, headers):
    limit = int(args["min_age"])
    return [u for u in data if u["age"] > limit]
```

```bash
fimod s -i users.json -m filter.py --arg min_age=30
```

### 🏷️ Dynamic Prefix/Suffix

```bash
fimod s -i data.json -e '{"msg": args["prefix"] + " " + data["name"]}' \
  --arg prefix="Hello"
```

## 🗂️ Dotpath Access

### 🔍 Read Nested Fields Safely

```python
# dp_get avoids KeyError for missing/optional fields
def transform(data, args, env, headers):
    city    = dp_get(data, "address.city", "unknown")
    country = dp_get(data, "address.country", "unknown")
    last    = dp_get(data, "items.-1")   # last array element
    return {"city": city, "country": country, "last_item": last}
```

### ✏️ Set Nested Fields

```python
# dp_set returns a new copy — original is unchanged
def transform(data, args, env, headers):
    data = dp_set(data, "meta.source", "fimod")
    data = dp_set(data, "meta.version", "1")
    return data
```

```bash
fimod s -i record.json -m enrich.py
```

## 🔁 Iteration Helpers

### 📂 Group Records by Field

```python
# it_group_by takes a field name (string), not a lambda
def transform(data, args, env, headers):
    return it_group_by(data, "department")
```

```bash
fimod s -i employees.json -m group.py --output-format json
# → {"engineering": [...], "sales": [...]}
```

### 📈 Sort Records by Field

```python
def transform(data, args, env, headers):
    return it_sort_by(data, "created_at")
```

### 🧹 Deduplicate by Field

```python
# Keep first occurrence, discard duplicates by email
def transform(data, args, env, headers):
    return it_unique_by(data, "email")
```

```bash
fimod s -i contacts.csv -m dedup.py -o contacts_clean.csv --output-format csv
```

### 🌀 Flatten Nested Arrays

```python
def transform(data, args, env, headers):
    # data = [[1, 2], [3, [4, 5]]]  →  [1, 2, 3, 4, 5]
    return it_flatten(data)
```

## #️⃣ Hashing for Anonymisation

### 🔒 Replace PII with SHA-256

```python
# hash_pii.py
# fimod: input-format=csv, output-format=csv
def transform(data, args, env, headers):
    for row in data:
        row["email"] = hs_sha256(row["email"])
        row["phone"] = hs_sha256(row["phone"])
    return data
```

```bash
fimod s -i users.csv -m hash_pii.py -o users_anon.csv
```

### 🔑 Generate Stable IDs from Keys

```python
def transform(data, args, env, headers):
    for row in data:
        key = f"{row['name']}|{row['dob']}"
        row["id"] = hs_md5(key)
    return data
```

## ✅ Validation with `--check`

### 🛡️ Validate a Config File

```python
# validate_config.py
def transform(data, args, env, headers):
    required = ["host", "port", "db"]
    return all(k in data and data[k] for k in required)
```

```bash
fimod s -i config.json -m validate_config.py --check
if [ $? -ne 0 ]; then
    echo "ERROR: config.json is missing required fields" >&2
    exit 1
fi
```

### 📋 Assert an API Response is Non-Empty

```bash
# Exit 1 if response array is empty or null
curl -s https://jsonplaceholder.typicode.com/todos | \
  fimod s --input-format json -e 'data and len(data) > 0' --check
```

## 🚫 Data Generation with `--no-input`

### 🏗️ Generate a Fixture from Arguments

```python
# gen_users.py
def transform(data, args, env, headers):
    n = int(args["count"])
    prefix = args.get("prefix", "user")
    return [{"id": i, "name": f"{prefix}{i}", "active": True} for i in range(1, n + 1)]
```

```bash
fimod s --no-input -m gen_users.py --arg count=10 --arg prefix="test" > users.json
```

### 📅 Emit a Timestamp Record

```bash
fimod s --no-input -e '{"generated_at": args["ts"], "env": args["env"]}' \
  --arg ts="2024-01-15T12:00:00Z" --arg env="production"
```

## 📥 Slurp Mode

### 🔗 Merge Multiple JSON Files

```bash
# Each file is a single JSON object — collect into an array
cat config-dev.json config-prod.json | fimod s --slurp -e 'data'
# → [{"env": "dev", ...}, {"env": "prod", ...}]
```

### 🗂️ Merge Config Files (Base + Overrides)

```bash
# base.yaml (defaults) + prod.toml (overrides) → merged JSON
fimod s -i base.yaml -i prod.toml --slurp -e '
def transform(data):
    data[0].update(data[1])
    return data[0]
' --output-format json
```

> **Note:** `{**a, **b}` and `a | b` are not supported in Monty. Use `a.update(b)` for dict merging.
