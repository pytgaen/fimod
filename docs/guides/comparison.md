# Feature Comparison

You already know Python. Why learn another DSL?

**jq / yq** — powerful but you need to learn a custom query language:

```bash
# jq: filter users older than 30
jq '[.[] | select(.age > 30)]' users.json

# fimod: same thing, it's just Python
fimod s -i users.json -e '[u for u in data if u["age"] > 30]'
```

```bash
# jq: project + sort + deduplicate
jq '[.[] | {id, name}] | sort_by(.name) | unique_by(.id)' data.json

# fimod: chain expressions, each feeds the next
fimod s -i data.json -e '[{"id": u["id"], "name": u["name"]} for u in data]' \
  -e 'it_unique_by(it_sort_by(data, "name"), "id")'
```

```bash
# yq: convert YAML → JSON (yq can't output TOML or CSV)
yq -o json config.yaml

# fimod: any format → any format
fimod s -i config.yaml -e 'data' -o config.toml
```

**Python one-liner** — works but painful boilerplate:

```bash
python3 -c "
import json, sys
data = json.load(sys.stdin)
print(json.dumps([u for u in data if u['active']]))
" < users.json

# fimod: same logic, zero boilerplate, no Python install
fimod s -i users.json -e '[u for u in data if u["active"]]'
```

## Matrix

| | **jq** | **yq** | **Python script** | **fimod** |
|---|:---:|:---:|:---:|:---:|
| **Language** | jq DSL | jq-like DSL | Python | 🟢 Python |
| **JSON** | ✅ | ✅ | manual I/O | 🟢 built-in |
| **YAML** | ❌ | ✅ | manual I/O | 🟢 built-in |
| **TOML** | ❌ | ✅ (read-only) | manual I/O | 🟢 built-in |
| **CSV** | ❌ | ✅ (limited) | manual I/O | 🟢 built-in |
| **NDJSON** | ✅ (--slurp) | ❌ | manual I/O | 🟢 built-in (--slurp) |
| **Cross-format** | ❌ | YAML↔JSON↔XML | manual | 🟢 any → any |
| **Dependencies** | jq binary | yq binary | Python + pip | 🟢 **single binary** |
| **Binary size** | ~2 MB | ~10 MB | ~30-100 MB (standalone) | 🟢 **~2.3 MB** (UPX-compressed) |
| **Regex** | limited | limited | `import re` | 🟢 `re_*` built-in (PCRE2) |
| **Deep access** | `.a.b.c` | `.a.b.c` | manual | 🟢 `dp_get(data, "a.b.c")` |
| **Group/sort/unique** | `group_by` | `group_by` | manual | 🟢 `it_group_by`, `it_sort_by`, `it_unique_by` |
| **Hashing** | ❌ | ❌ | `import hashlib` | 🟢 `hs_sha256`, `hs_md5`, `hs_sha1` |
| **In-place edit** | sponge hack | `-i` | manual | 🟢 `--in-place` |
| **Batch files** | loop | loop | loop | 🟢 `fimod s -i *.json -m t.py -o out/` |
| **Chaining** | `|` (inside jq) | `|` (inside yq) | manual | 🟢 `-e expr1 -e expr2` |
| **Exit codes** | ❌ | ❌ | `sys.exit()` | 🟢 `--check` + `set_exit()` |
| **Reusable scripts** | ❌ | ❌ | yes | 🟢 mold scripts + registry |
| **Remote scripts** | ❌ | ❌ | ❌ | 🟢 `-m https://...` |
| **HTTP input** | ❌ | ❌ | `requests` + boilerplate | 🟢 `-i https://...` (replaces curl) |
| **Test runner** | ❌ | ❌ | pytest | 🟢 `fimod test mold.py tests/` |
