# YAML — Practical Examples

## Types in YAML

YAML booleans, nulls, and numbers map naturally to JSON types:

```bash
printf 'active: true\ncount: 42\nlabel: null\n' | \
  fimod s --input-format yaml -e 'data' --output-format json
```
```json
{"active": true, "count": 42, "label": null}
```

---

## Convert to other formats

```bash
# YAML → JSON
printf 'host: localhost\nport: 5432\ndb: myapp\n' | \
  fimod s --input-format yaml -e 'data' --output-format json

# YAML → TOML
printf 'host: localhost\nport: 5432\ndb: myapp\n' | \
  fimod s --input-format yaml -e 'data' --output-format toml
```

For a file:

```bash
cat > /tmp/config.yaml << 'EOF'
host: localhost
port: 5432
db: myapp
EOF

fimod s -i /tmp/config.yaml -e 'data' --output-format json
fimod s -i /tmp/config.yaml -e 'data' -o /tmp/config.toml
```

!!! warning "TOML requires a root-level object"
    Arrays or scalars at the root will fail to serialize as TOML — TOML spec constraint.

---

## Normalize / reformat in-place

```bash
cat > /tmp/config.yaml << 'EOF'
host:   localhost
port:  5432
db:    myapp
EOF

fimod s -i /tmp/config.yaml -e 'data' --in-place
cat /tmp/config.yaml
```
```yaml
host: localhost
port: 5432
db: myapp
```

---

## Filter a list

```bash
printf 'users:\n  - name: Alice\n    role: admin\n  - name: Bob\n    role: user\n' | \
  fimod s --input-format yaml \
  -e '[u for u in data["users"] if u["role"] == "admin"]' \
  --output-format yaml
```
```yaml
- name: Alice
  role: admin
```

---

## Edit a nested value

```bash
cat > /tmp/config.yaml << 'EOF'
database:
  host: localhost
  port: 5432
EOF

fimod s -i /tmp/config.yaml -e 'dp_set(data, "database.port", 5433)' --output-format yaml
```
```yaml
database:
  host: localhost
  port: 5433
```

---

## YAML array → CSV

```bash
printf '- name: Alice\n  age: 30\n- name: Bob\n  age: 25\n' | \
  fimod s --input-format yaml -e 'data' --output-format csv
```
```
name,age
Alice,30
Bob,25
```
