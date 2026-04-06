# 🗺️ Quick Tour

A 5-minute showcase of what you can do with `fimod`.

## ⚡ One-liners with `-e`

The quickest way to transform data. The expression receives `data` and returns the result:

```bash
# 🔍 Filter active users
fimod s -i users.json -e '[u for u in data if u["active"]]'

# 🔄 Reshape a record
fimod s -i users.json -e '{"name": data[0]["name"].upper(), "count": len(data)}'

# 🔗 Chain multiple expressions (output of each feeds the next)
fimod s -i data.json \
  -e '[u for u in data if u["active"]]' \
  -e 'it_sort_by(data, "name")' \
  -e '{"users": data, "count": len(data)}'
```

For multi-statement transforms, write `def transform` inside `-e`:

```bash
fimod s -i data.json -e '
def transform(data, args, env, headers):
    result = {}
    for item in data:
        result[item["id"]] = item["name"]
    return result
'
```

## 📜 Reusable scripts (`-m`)

For reusable transforms, write a `transform(data, args, env, headers)` function in a `.py` file:

```python
# cleanup.py
def transform(data, args, env, headers):
    for row in data:
        row["name"] = row["name"].strip().title()
        row["email"] = row["email"].strip().lower()
    return it_unique_by(data, "email")
```

```bash
fimod s -i contacts.csv -m cleanup.py -o contacts.json
```

## 🔀 Cross-format conversion

Formats are auto-detected from file extensions. A pass-through expression converts between formats:

```bash
fimod s -i config.yaml -e 'data' -o config.toml       # YAML → TOML
fimod s -i data.csv -e 'data' --output-format json     # CSV → JSON
fimod s -i users.json -e 'data' --output-format ndjson  # JSON → NDJSON
```

## 🔥 HTTP Input

The `-i` flag accepts URLs just like file paths — fimod fetches, parses, and transforms in one command (no `curl | jq` needed):

```bash
# Fetch and transform in one shot
fimod s -i https://api.github.com/repos/pytgaen/fimod \
    -e '{"name": data["name"], "stars": data["stargazers_count"]}'

# With custom headers for authenticated APIs
fimod s -i https://api.github.com/user/repos \
    --http-header "Authorization: Bearer $GITHUB_TOKEN" \
    -e '[r["full_name"] for r in data]'
```

When you need more than the body — status codes, headers, or redirects — use
[`--input-format http`](../examples/http.md).

## 🌐 Remote scripts

Load scripts directly from URLs — no local file needed:

```bash
fimod s -i data.json -m https://example.com/transforms/normalize.py
```

## 🧰 Built-in helpers — no imports needed

```python
# 🔍 Regex (fancy-regex: lookahead, lookbehind, backrefs, atomic groups)
# re_sub uses Python \1/\g<name> syntax; re_sub_fancy uses $1/${name}
emails = re_findall(r"\w+@\w+\.\w+", text)
cleaned = re_sub(r"\s+", " ", text)
swapped = re_sub(r"(\w+)@(\w+)", r"\2/\1", text)        # Python syntax
swapped = re_sub_fancy(r"(\w+)@(\w+)", "$2/$1", text)   # fancy syntax

# 🗂️ Deep access into nested structures
city = dp_get(data, "users.0.address.city", "unknown")
data = dp_set(data, "meta.processed", True)

# 🔁 Collections
grouped = it_group_by(data, "department")
sorted_list = it_sort_by(data, "created_at")
unique = it_unique_by(data, "email")

# #️⃣ Hashing for anonymization
anon_email = hs_sha256(user["email"])
```

## 🎛️ Parameterized scripts

```bash
# Pass named arguments → available as `args` dict
fimod s -i users.json --arg min_age=30 --arg dept=engineering \
  -e '[u for u in data if u["age"] > int(args["min_age"]) and u["dept"] == args["dept"]]'
```

## 🐚 Shell integration

```bash
# ✅ --check: exit 0 if truthy, 1 if falsy (like grep -q for structured data)
if fimod s -i config.json -e '"host" in data and "port" in data' --check; then
    echo "Config OK"
fi

# 📝 --output-format txt: strings without JSON quotes (for shell variables)
NAME=$(fimod s -i user.json -e 'data["name"]' --output-format txt)

# 🔗 Pipe-friendly: reads stdin, writes stdout
curl -s https://jsonplaceholder.typicode.com/users | fimod s -e '[u["email"] for u in data]'
```

## 📦 Batch processing

```bash
# Process all JSON files, output to a directory
fimod s -i data/*.json -m normalize.py -o cleaned/

# In-place editing of multiple files
fimod s -i configs/*.yaml -e 'dp_set(data, "version", "2.0")' --in-place
```

## 📚 Mold registry

Organize and share your transforms. Register local directories or remote Git repositories as named sources, then reference molds by name with `@`.

```bash
# ➕ Register sources (local or remote)
fimod registry add my-molds ./transforms/
fimod registry add company https://github.com/org/fimod-molds
fimod registry add private https://gitlab.com/team/molds --token-env GITLAB_TOKEN

# 📋 Manage your registries
fimod registry list
fimod registry show company
fimod registry set-priority company 0             # set as P0 (highest priority)
fimod registry set-priority private 1            # set as P1

# 🚀 Use registered molds with @name
fimod s -i data.json -m @cleanup                  # searches all registries in priority order
fimod s -i data.json -m @company/normalize        # from specific registry
fimod s -i data.json -m @my-molds/csv-to-json     # from local directory

# 🔒 Token authentication auto-detected for GitHub / GitLab / Gitea
#    or set manually with --token-env for custom hosts
```

The registry config lives in `~/.config/fimod/sources.toml` — one file, human-readable, version-controllable.

## 🧪 Built-in test runner

Write test cases as `*.input.<ext>` / `*.expected.<ext>` file pairs, and fimod validates your mold produces the expected output:

```bash
fimod mold test cleanup.py tests/
# ✅ 001 ... ok
# ✅ 002 ... ok
# ❌ 003 ... FAILED
#   expected: {"name": "Alice"}
#   got:      {"name": "alice"}
```

## 🌍 Environment variables

fimod sandboxes env vars — you choose exactly what to expose:

```bash
# Inject specific env vars (glob patterns, comma-separated)
fimod s --env 'CI,GITHUB_*' -i data.json -e '{"app": data, "ci": env.get("CI", "false"), "branch": env.get("GITHUB_REF", "local")}'

# Inject all env vars starting with P (at least PATH is always present)
fimod s --env 'P*' --no-input -e 'len(env)'  # → ≥1

# Inject all env vars
fimod s --env '*' -i data.json -e '{"user": env.get("USER", "unknown")}'

# Without --env, env is an empty dict {}
fimod s -i data.json -e 'len(env)'  # → 0
```

## 💼 Real-world examples

**Patch a Kubernetes manifest before deploy:**

```bash
fimod s -i deployment.yaml -m @yaml_merge \
  --arg set="spec.replicas=3,metadata.labels.env=prod" \
  -o deployment.yaml
```

**Validate required config fields in CI:**

```bash
fimod s -i config.json -m @validate_fields --arg required=database.host,database.port,api.key
# → exits 1 if any field is missing
```

**📊 API response → flat CSV for a spreadsheet:**

```bash
# No curl needed — fimod fetches directly
fimod s -i https://jsonplaceholder.typicode.com/users \
    -e '[{"id": u["id"], "name": u["name"], "email": u["email"], "city": dp_get(u, "address.city"), "company": dp_get(u, "company.name")} for u in data]' \
    -o contacts.csv
```

**🔒 Anonymize PII in a CSV export:**

```python
# anonymize.py
# fimod: input-format=csv, output-format=csv
def transform(data, args, env, headers):
    for row in data:
        row["email"] = hs_sha256(row["email"])
        row["phone"] = hs_sha256(row["phone"])
        row["name"] = row["name"][0] + "***"
    return data
```

```bash
fimod s -i users.csv -m anonymize.py -o users_safe.csv
```

**📋 Merge NDJSON logs and extract errors:**

```bash
cat app-*.log | fimod s --input-format ndjson --slurp \
  -e '[l for l in data if l["level"] == "ERROR"]' \
  -e 'it_sort_by(data, "timestamp")' \
  -o errors.json
```

**✅ Validate CI config before deploy:**

`--check` exits 0 if the expression is truthy, 1 if falsy — no output, like `grep -q` for structured data:

```bash
fimod s -i deploy.yaml \
  -e 'all(k in data for k in ["image", "replicas", "port"]) and int(data["replicas"]) > 0' \
  --check || { echo "Invalid deploy config" >&2; exit 1; }
```

Better: use `gk_assert` for precise error messages and automatic exit — no shell boilerplate needed:

```bash
fimod s -i deploy.yaml \
  -e 'gk_assert(all(k in data for k in ["image", "replicas", "port"]), "missing required fields: image, replicas, port")' \
  -e 'gk_assert(int(data["replicas"]) > 0, "replicas must be > 0")' \
  -e 'data'
```

Each assertion prints a specific error to stderr and exits with a non-zero code on failure.

**🔄 CI/CD: share transforms across repos:**

```yaml
# .github/workflows/transform.yml
- name: Transform data
  env:
    FIMOD_REGISTRY: "https://github.com/my-org/fimod-molds/tree/main/ci"
  run: |
    fimod s -i data.json -m @normalize -o output.json
```

> Looking for `@gh_latest`, `@download`, `@poetry_migrate`, `@skylos_to_gitlab`? See the [fimod-powered](https://github.com/pytgaen/fimod-powered) registry.

## 📝 Data→Text: Jinja2 Templating

Fimod isn't limited to data→data transforms. With `tpl_render_str` and `tpl_render_from_mold`, you can generate **any text file** from structured data — configs, Dockerfiles, reports, k8s manifests, CI workflows.

**Quick one-liner — generate a `.env` file:**

```bash
echo '{"host":"db.prod","port":5432,"secret":"s3cret"}' \
  | fimod s -e 'tpl_render_str("{% for k, v in data | items %}{{ k | upper }}={{ v }}\n{% endfor %}", data)' \
    --output-format txt
```
```
HOST=db.prod
PORT=5432
SECRET=s3cret
```

**Inline template — Dockerfile from a config:**

```bash
echo '{"python_version":"3.12","packages":["flask","gunicorn"]}' \
  | fimod s --output-format txt -e '
tpl_render_str("""
FROM python:{{ python_version }}-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir {{ packages | join(" ") }}
COPY . .
""".strip(), data)'
```
```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir flask gunicorn
COPY . .
```

**Production mold with `.j2` template files:**

For real projects, separate logic from presentation. Create a directory mold with templates:

```
my_mold/
├── my_mold.py
└── templates/
    └── nginx.conf.j2
```

```python
# my_mold/my_mold.py
"""Generate nginx config from a service descriptor."""
# fimod: output-format=txt

def transform(data, args, env, headers):
    return tpl_render_from_mold("templates/nginx.conf.j2", data)
```

```jinja
{# templates/nginx.conf.j2 #}
{% for svc in services %}
upstream {{ svc.name }} {
    {% for host in svc.hosts %}
    server {{ host }}:{{ svc.port | default(8080) }};
    {% endfor %}
}
{% endfor %}

server {
    listen 80;
    {% for svc in services %}
    location /{{ svc.name }}/ {
        proxy_pass http://{{ svc.name }};
    }
    {% endfor %}
}
```

```bash
fimod s -i services.yaml -m ./my_mold/ -o nginx.conf
```

**Registry mold — changelog from git log:**

The `@git_changelog` example mold uses `tpl_render_from_mold` with a `.j2` template to generate a Markdown changelog:

```bash
echo '[{"hash":"e813bcb","msg":"feat: add templating","date":"2026-03-29"},
      {"hash":"48637d1","msg":"feat: add registry cache","date":"2026-03-28"}]' \
  | fimod s -m @git_changelog --output-format txt
```
```markdown
# Changelog

2 commits.

## 2026-03-29

- feat: add templating (e813bcb)

## 2026-03-28

- feat: add registry cache (48637d1)
```

Full Jinja2 syntax is available: loops, conditions, filters (`upper`, `join`, `tojson`, `default`, …), macros, and template inheritance. See [Built-ins Reference](../reference/built-ins.md#template-functions-tpl) for the full API.
