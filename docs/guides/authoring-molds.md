# 🛠️ Authoring Molds

A practical guide for writing, testing, and publishing mold scripts.

---

## ⚡ Quick iteration loop

The fastest way to develop a mold is a tight write → run → observe cycle.

**1. Start with inline `-e`** to validate the logic before writing a file:

```bash
fimod s -i data.json -e '[u for u in data if u["active"]]'
```

**2. Move to a `.py` file** once the expression grows:

```python
# cleanup.py
def transform(data, args, env, headers):
    return [u for u in data if u["active"] and u.get("role") == "admin"]
```

```bash
fimod s -i data.json -m cleanup.py
```

**3. Auto-rerun on save** using `watchexec` or `entr`:

```bash
# watchexec (https://github.com/watchexec/watchexec)
watchexec -w cleanup.py -w data.json -- fimod s -i data.json -m cleanup.py

# entr (available on most Linux/macOS)
ls cleanup.py data.json | entr fimod s -i data.json -m cleanup.py
```

Every time you save `cleanup.py`, fimod reruns and prints the result.

---

## 🔍 Inspect what's happening with `--debug`

`--debug` prints the full execution trace to stderr — input data, script, and output:

```bash
fimod s -i data.json -m cleanup.py --debug
```

```
[debug] input format:  json
[debug] output format: json
[debug] script:
  def transform(data, args, env, headers):
      return [u for u in data if u["active"]]
[debug] input data:
  [{"name": "Alice", "active": true}, {"name": "Bob", "active": false}]
[debug] output data:
  [{"name": "Alice", "active": true}]
```

!!! tip
    `--debug` writes to stderr, so you can pipe stdout normally:
    ```bash
    fimod s -i data.json -m cleanup.py --debug | jq '.[0]'
    ```

---

## 📝 Diff input vs output

To see exactly what your mold changes, diff the input against the output:

```bash
diff <(cat data.json) <(fimod s -i data.json -m cleanup.py)
```

For pretty JSON diffs:

```bash
# Using diff with sorted JSON
diff \
  <(python3 -m json.tool data.json) \
  <(fimod s -i data.json -m cleanup.py | python3 -m json.tool)
```

Useful before applying `--in-place` to verify the transform is safe.

---

## ✅ Validate with `fimod test`

Once your mold is working, write test cases to lock in the expected behaviour.

Create a directory with `<name>.input.<ext>` / `<name>.expected.<ext>` pairs:

```
tests/
├── basic.input.json
├── basic.expected.json
├── edge-empty.input.json
└── edge-empty.expected.json
```

Run the tests:

```bash
fimod mold test cleanup.py tests/
```

```
  ✓ basic
  ✓ edge-empty

2 tests passed
```

Exit code is `0` if all pass, `1` if any fail — suitable for CI.

!!! tip "Input and expected can use different formats"
    `basic.input.csv` + `basic.expected.json` works — fimod detects formats from extensions.

### Parameterized cases with `.run-test.toml`

For molds that require `--arg`, `--env`, or a specific exit code, add a `{case}.run-test.toml` alongside the input/expected files:

```
tests/
├── basic.input.json
├── basic.expected.json
├── basic.run-test.toml       ← args, env, exit code for this case
├── missing.input.json
├── missing.expected.json
└── missing.run-test.toml     ← expects exit code 1
```

```toml
# basic.run-test.toml
[args]
field = "email"
value = "admin"
```

```toml
# missing.run-test.toml
exit_code = 1
```

All supported keys:

```toml
[args]               # --arg key=value pairs passed to the mold
field = "email"

[env_vars]           # environment variables injected for this run
MY_PREFIX_FOO = "bar"

exit_code = 1        # expected exit code (default: 0)
input_format = "json"   # override auto-detected input format
output_format = "json-compact"  # override output format
input_file = "other.input.json"  # use a different input file
skip = true          # skip this case
```

---

## 🎛️ Parameterize with `--arg`

Avoid hardcoding values — use `args` to make your mold reusable:

```python
# filter.py
def transform(data, args, env, headers):
    field = args["field"]
    value = args["value"]
    return [row for row in data if row.get(field) == value]
```

```bash
fimod s -i users.json -m filter.py --arg field="role" --arg value="admin"
fimod s -i orders.json -m filter.py --arg field="status" --arg value="pending"
```

Use `.run-test.toml` to test parameterized molds (see above).

---

## 📋 Embed defaults with `# fimod:`

If your mold always expects CSV input or compact JSON output, declare it at the top of the file:

```python
"""Convert CSV scores to JSON."""
# fimod: input-format=csv, output-format=json
def transform(data, args, env, headers):
    return [{"name": row["name"], "score": int(row["score"])} for row in data]
```

Users can still override with explicit CLI flags. See [Mold Defaults](../reference/mold-defaults.md).

The module-level docstring (`"""..."""`) is used by `fimod mold list` to describe the mold in registries.

---

## 📦 Register in a local registry

Once the mold is ready, place it in a directory registered as a source so you can call it by name from anywhere:

```bash
# Register the directory as a registry (once)
fimod registry add my ~/molds/

# Copy or move the mold there
cp ./normalize.py ~/molds/

# Use by name — no path needed
fimod s -i data.json -m @normalize        # searches all registries in priority order
fimod s -i data.json -m @my/normalize     # explicit registry name
```

List the molds available in a registry:

```bash
fimod mold list           # all registries
fimod mold list my        # named registry
```

If you publish the registry remotely (GitHub, GitLab, HTTP), generate a `catalog.toml` first so remote users can browse it:

```bash
fimod registry build-catalog ./molds         # scans directory and writes catalog.toml
fimod registry build-catalog --registry my   # same, resolving path from a registered registry
# commit and push catalog.toml alongside the molds
```

---

## 🗂️ Suggested project layout

For a project that ships its own molds:

```
my-project/
├── data/
│   └── raw.json
├── molds/
│   ├── normalize.py
│   ├── aggregate.py
│   └── export.py
└── tests/
    ├── normalize/
    │   ├── basic.input.json
    │   └── basic.expected.json
    └── aggregate/
        ├── dept.input.json
        └── dept.expected.json
```

Run all mold tests in CI:

```bash
fimod test molds/normalize.py tests/normalize/
fimod test molds/aggregate.py tests/aggregate/
```
