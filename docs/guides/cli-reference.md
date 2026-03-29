# 🖥️ CLI Reference

## Usage

```bash
fimod s -i <INPUT> -m <MOLD> [OPTIONS]
fimod s -i <INPUT> -e '<EXPRESSION>' [OPTIONS]
fimod s --no-input -m <MOLD> [OPTIONS]
```

!!! note
    Either `-m` or `-e` is required (but not both).

---

## 🎯 Core options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--input` | `-i` | Input file path or URL (`https://...`) | `stdin` |
| `--mold` | `-m` | Path, URL, or registered name of the transform script | — |
| `--expression` | `-e` | Inline Python expression (mutually exclusive with `-m`) | — |
| `--output` | `-o` | Output file path | `stdout` |
| `--in-place` | | Modify input file in-place (requires `-i`, incompatible with `-o` and URLs) | — |
| `--slurp` | `-s` | Read multiple JSON values into a single array | — |
| `--no-input` | | Skip input — `data = None` in Python | — |
| `--check` | | No stdout; exit 0 if result is truthy, 1 if falsy | — |
| `--debug` | `-d` | Print pipeline diagnostics to stderr | — |

---

## 📄 Format control

| Option | Description |
|--------|-------------|
| `--input-format` | Explicitly set input format. Auto-detected from extension if omitted. |
| `--output-format` | Explicitly set output format. Defaults to input format if omitted. |

Supported values: `json` · `json-compact` · `ndjson` · `yaml` · `toml` · `csv` · `txt` · `lines` · `raw` (output-only) · `http` (input-only)

### 🔍 Auto-detection

| Extension | Format |
|-----------|--------|
| `.json` | JSON |
| `.ndjson`, `.jsonl` | NDJSON |
| `.yaml`, `.yml` | YAML |
| `.toml` | TOML |
| `.csv`, `.tsv` | CSV |
| `.txt`, `.text` | TXT |
| *(none / stdin)* | JSON (default) |

!!! warning "Lines is never auto-detected"
    Always use `--input-format lines` explicitly.

---

## 🔥 Your input can be an HTTPS request!

Awesome: **the input can be an HTTPS request.** The `-i` flag accepts URLs exactly like file paths — fimod fetches, parses, and transforms in a single command. No `curl`, no `wget`, no pipes.

```bash
# Fetch JSON from an API
fimod s -i https://api.github.com/repos/pytgaen/fimod -e 'data["name"]' --output-format txt

# Multiple URLs in batch mode
fimod s -i https://jsonplaceholder.typicode.com/users/1 https://jsonplaceholder.typicode.com/users/2 -e 'data' -o responses/
```

**Format auto-detection**: the response's `Content-Type` header is used as fallback when `--input-format` is not set (`application/json` → JSON, `text/csv` → CSV, etc.). You can always override explicitly.

!!! warning "`--in-place` is not allowed with URLs"

### HTTP options

| Option | Description | Default |
|--------|-------------|---------|
| `--header "Name: Value"` | Custom HTTP header (repeatable) | — |
| `--timeout <seconds>` | Request timeout | `30` |
| `--no-follow` | Don't follow HTTP redirects | follows redirects |
| `--no-cache` | Bypass local cache for remote catalogs and molds | uses cache |

```bash
# Authenticated API
fimod s -i https://api.github.com/user \
    --http-header "Authorization: Bearer $GITHUB_TOKEN" \
    -e 'data["login"]' --output-format txt

# Disable redirects to inspect the Location header
fimod s -i https://github.com/pytgaen/fimod/releases/latest \
    --input-format http --no-follow \
    -e 'data["headers"]["location"]' --output-format txt
```

### `--input-format http` — raw HTTP response

When you need the full HTTP response (status code, headers, body), use `--input-format http`. Instead of parsing the body, fimod gives you a dict:

```python
data = {
    "status": 200,
    "headers": {"content-type": "application/json", "location": "..."},
    "body": "..."  # raw response body as a string
}
```

```bash
# Get the redirect target of a URL
fimod s -i https://github.com/pytgaen/fimod/releases/latest \
    --input-format http --no-follow \
    -e 'data["headers"]["location"].split("/")[-1]' --output-format txt
# → v0.3.0

# Combine with set_input_format() to re-parse the body
fimod s -i https://api.github.com/repos/pytgaen/fimod/releases/latest \
    --input-format http \
    -e 'set_input_format("json"); data["body"]' \
    -e 'data["tag_name"]' --output-format txt
```

!!! info "`http` is input-only"
    Using `--output-format http` will produce an error. HTTP is only meaningful as an input format.

!!! tip "Requires the `full` build variant"
    HTTP input needs reqwest, which is only included in `FIMOD_VARIANT=full` or `cargo build --features full`.

---

## 📥 Slurp mode (`--slurp` / `-s`)

`-s` has two distinct uses depending on whether you pass a single input or multiple `-i` entries.

### Single input — NDJSON / concatenated JSON

Reads **multiple concatenated JSON values** from a single source and collects them into an array.

```bash
# 🔗 Two separate JSON objects → array of two
printf '{"a":1}\n{"b":2}' | fimod s --slurp -e 'len(data)'
# → 2

# 📂 Combine multiple JSON files via cat (JSON only)
cat file1.json file2.json | fimod s --slurp -e '[item for item in data if item.get("active")]'
```

For non-JSON formats, `--slurp` wraps the single parsed value in a one-element array.

### Multiple inputs — multi-file slurp

When combined with **two or more `-i` flags**, `-s` switches to **multi-file slurp** mode: all files are parsed (each in their own format) and combined into a single `data` structure. The mold runs **once** against the combined result.

Two sub-modes controlled by the `-i` syntax:

| Syntax | Mode | `data` shape |
|--------|------|-------------|
| `-i f1 -i f2` | **list** | `[parsed_f1, parsed_f2]` |
| `-i f1: -i f2:` | **named (auto stem)** | `{"f1": parsed_f1, "f2": parsed_f2}` |
| `-i f1:alias -i f2:other` | **named (explicit)** | `{"alias": parsed_f1, "other": parsed_f2}` |

Rules:
- All entries must use `:` or none must (mixing → error)
- Auto stem uses the filename without extension; duplicate stems → error (use explicit aliases)
- Incompatible with `--in-place` and `-o <directory>`

```bash
# ➕ List mode — access by index, cross-format (JSON + YAML)
fimod s -i base.json -i override.yaml -s -e 'data[1]["env"]'

# 🏷️ Named mode — auto stem as key
fimod s -i defaults.json: -i prod.toml: -s \
  -e 'data["defaults"]["timeout"]'

# 🏷️ Named mode — explicit aliases (resolves stem collisions)
fimod s -i config/base.json:base -i other/base.json:fallback -s \
  -e 'data["fallback"]["host"]'

# 🔍 Diff two files (with df_ helpers, once implemented)
fimod s -i before.json:old -i after.json:new -s \
  -e 'df_diff(data["old"], data["new"])'

# 💾 Write result to a file
fimod s -i a.yaml -i b.yaml -s -e 'data[0]' -o merged.yaml
```

---

## 🚫 No-input mode (`--no-input`)

Skip input entirely — `data` is `None` in Python. Use to generate data from scratch.

!!! info "Incompatible with"
    `-i`, `--input-format`, `--in-place`

```bash
fimod s --no-input -e '{"status": "ok", "ts": args["ts"]}' --arg ts="2024-01-01"

fimod s --no-input -m generate.py --arg count=5
```

```python
# generate.py
def transform(data, args, env, headers):
    n = int(args["count"])
    return [{"id": i, "value": i * i} for i in range(n)]
```

---

## ✅ Check mode (`--check`)

Suppresses stdout and uses the **truthiness** of the result as the exit code.

- **Exit 0** — result is truthy
- **Exit 1** — result is falsy

See [Exit Codes](../reference/exit-codes.md) for the full truthiness table.

```bash
# ✅ Validate a record
fimod s -i record.json -e 'data.get("email") and data.get("name")' --check

# 🔀 Use in a shell conditional
if fimod s -i config.json -m validate.py --check; then
    echo "✅ Config is valid"
else
    echo "❌ Config has errors" >&2
    exit 1
fi
```

!!! tip
    When a mold calls `set_exit(code)`, that code takes priority over `--check` truthiness.

---

## 🗂️ Mold registries

Registries are named collections of molds (local directories or remote repos).
Use `@name` or `@registry/name` with `-m` to reference a mold by name.

```bash
fimod registry setup                                     # 🚀 First-run setup (official catalog)
fimod registry add my ~/molds/                          # ➕ Add local registry
fimod registry add official https://github.com/org/molds # ➕ Add GitHub registry
fimod registry list                                      # 📋 List registries
fimod registry show my                                   # 🔍 Show details
fimod registry remove my                                 # 🗑️ Remove
fimod registry set-default official                      # ⭐ Set default (P0)
fimod registry set-default --clear                       # ⭐ Clear default
fimod registry set-priority mycompany 1                  # 📌 Set priority P1
fimod registry set-priority mycompany --clear            # 📌 Clear priority
fimod registry build-catalog my                          # 📦 Generate catalog.toml
fimod registry cache info                                # 📊 Show cache location and usage
fimod registry cache clear                               # 🧹 Clear all cached catalogs and molds
```

Config stored in `~/.config/fimod/sources.toml`. Cache stored in `~/.cache/fimod/`.

### `registry setup` — first-run onboarding

`fimod registry setup` adds the official fimod mold catalog if not already present. The install scripts call it automatically, but it is safe to run manually at any time (idempotent).

```bash
fimod registry setup           # interactive — asks before adding
fimod registry setup --yes     # non-interactive / CI — skips the prompt
fimod registry setup --force   # promote official registry to default even if another default exists
```

Behaviour summary:

| Situation | Without `--force` | With `--force` |
|---|---|---|
| Already configured | "already configured", no-op | same |
| No default registry yet | adds as default (fresh install) | same |
| Default already exists | adds without changing default | adds and promotes to default |
| Non-interactive (no TTY) | skips silently | skips silently |

### Priority-based resolution

When you use `@mold` (without a registry prefix), fimod searches **all** configured registries in priority order until the mold is found:

| Priority | Source |
|----------|--------|
| P0 | `FIMOD_REGISTRY` anonymous entries (env always wins) |
| P0 | `default` registry from `sources.toml` |
| P1, P2, … | Registries listed in `[priority]` section |
| — | Remaining registries (file order, after all prioritized ones) |

```bash
$ fimod registry list
  official    [github]  github.com/org/fimod-powered       P0 (default)
  mycompany   [http]    registry.corp.com/fimod             P1
  internal    [local]   /home/user/molds                    P2
```

With `@registry/mold` (explicit prefix), only that specific registry is searched.

#### `set-default` — set or clear the default registry (P0)

```bash
fimod registry set-default official     # Set 'official' as P0
fimod registry set-default --clear      # Remove the P0 default
```

The default registry is always P0. If a registry was previously in `[priority]`, it is removed from there when promoted to default.

#### `set-priority` — assign a priority rank

```bash
fimod registry set-priority mycompany 1    # Set mycompany to P1
fimod registry set-priority internal 2     # Set internal to P2
fimod registry set-priority mycompany --clear  # Remove priority
```

If the requested rank is already taken, existing entries shift up automatically:

```bash
# Before: mycompany=P1, internal=P2
$ fimod registry set-priority newreg 1
# After:  newreg=P1, mycompany=P2, internal=P3
```

A registry that is already default (P0) cannot be assigned a priority — use `set-default --clear` first.

#### `sources.toml` format

```toml
default = "official"

[priority]
mycompany = 1
internal = 2

[sources.official]
type = "github"
url = "https://github.com/org/fimod-powered"

[sources.mycompany]
type = "http"
url = "https://registry.corp.com/fimod"

[sources.internal]
type = "local"
path = "/home/user/molds"
```

The `[priority]` section is optional. Without it, the default registry is searched first, then the rest in file order.

### `FIMOD_REGISTRY` — ephemeral registries for CI

In CI/CD or ephemeral environments, use the `FIMOD_REGISTRY` environment variable instead of `fimod registry add`. Comma-separated entries, optionally named:

```bash
# Anonymous — resolves @mold
FIMOD_REGISTRY=./molds fimod s -i data.json -m @clean

# Named — resolves @ci/mold and @mold
FIMOD_REGISTRY="ci=./molds,staging=https://github.com/org/molds" fimod s -i data.json -m @ci/clean

# Mixed anonymous + named
FIMOD_REGISTRY="ci=./molds,/opt/shared-molds" fimod s -i data.json -m @clean
```

`FIMOD_REGISTRY` takes priority over `sources.toml` (env overrides config, standard Unix convention). Named entries support `@name/mold` resolution.

```bash
# Register a local collection once…
fimod registry add my ~/molds/

# …then reference molds by name
fimod s -i messy.csv -m @cleanup          # searches all registries in priority order
fimod s -i messy.csv -m @my/cleanup       # explicit registry
```

**Authentication** — tokens are resolved automatically, or overridden per-registry:

| Source type | Default token |
|---|---|
| `github.com` | `$GITHUB_TOKEN` |
| GitLab | `$GITLAB_TOKEN` |
| Other HTTP (Gitea, Forgejo, …) | `$FIMOD_DL_AUTH_TOKEN` |
| Custom | `--token-env MY_VAR` at `registry add` time |

### GitHub URL formats

fimod accepts different URL forms depending on the Git ref type you want to pin to:

| Ref type | URL to pass to `registry add` |
|---|---|
| Branch (`main`, `dev`, …) | `https://github.com/org/repo/tree/main/molds` |
| Semver tag (`v1.0.0`, `2.3.4`, …) | `https://github.com/org/repo/tree/v1.0.0/molds` |
| Other tag (`stable`, `latest`, …) | `https://raw.githubusercontent.com/org/repo/refs/tags/stable/molds` |
| Commit SHA | `https://github.com/org/repo/tree/abc1234.../molds` |

For branches and semver tags, pass the standard `github.com/tree/…` URL — fimod converts it to a raw URL automatically, using `refs/heads/` for branches and `refs/tags/` for semver-looking refs.

For **non-semver tags** (e.g. `stable`, `latest`), fimod cannot distinguish them from branch names based on the ref string alone. Pass the `raw.githubusercontent.com` URL with the full ref path instead:

```bash
# Branch — standard github.com URL
fimod registry add mylib https://github.com/org/repo/tree/main/molds

# Semver tag — standard github.com URL
fimod registry add mylib https://github.com/org/repo/tree/v1.2.0/molds

# Non-semver tag — raw URL required
fimod registry add mylib https://raw.githubusercontent.com/org/repo/refs/tags/stable/molds
```

### Browsing available molds

```bash
fimod mold list              # list molds in all registries
fimod mold list official     # list molds in a specific registry
```

For **local** registries, mold names and descriptions are discovered by scanning `.py` files.
For **remote** registries (GitHub, GitLab, HTTP), a `catalog.toml` must be present at the root of the registry.

Maintainers generate it with:

```bash
fimod registry build-catalog my   # scans ~/molds/ and writes catalog.toml
```

The `catalog.toml` is a simple TOML file committed alongside the molds:

```toml
[molds.normalize]
description = "Normalise field names to snake_case"

[molds.filter_active]
description = "Keep only active records"
```

Mold descriptions come from the module-level docstring at the top of each script:

```python
"""Normalise field names to snake_case."""
def transform(data, args, env, headers):
    ...
```

### Cache management

Remote catalogs and molds are cached locally in `~/.cache/fimod/` to avoid re-fetching on every invocation.

**Catalog caching** uses HTTP ETags: on subsequent requests, fimod sends `If-None-Match` and skips the download on `304 Not Modified`.

**Mold caching** uses content hashes from the catalog: `build-catalog` computes a hash per mold directory, and the client only re-downloads when the hash changes.

```bash
fimod registry cache info              # show cache location and disk usage
fimod registry cache clear             # wipe all cached data
fimod registry cache clear @name       # wipe a specific mold (planned)
fimod s -i data.json -m @mold --no-cache  # bypass cache for this invocation
```

Override the cache directory with `FIMOD_CACHE_DIR`. For direct-URL molds (not registry-based), cache TTL is controlled by `FIMOD_CACHE_TTL` (minutes, default `360`, `0` = infinite, negative = disabled).

---

## 📊 CSV options

| Option | Description |
|--------|-------------|
| `--csv-delimiter <char>` | Separator character for input (default: `,`). Use `\t` for tabs. |
| `--csv-output-delimiter <char>` | Separator for output (defaults to `--csv-delimiter`). |
| `--csv-no-input-header` | First line is data, not header. Columns: `col0`, `col1`, ... |
| `--csv-no-output-header` | Don't write header row in output. |
| `--csv-header "a,b,c"` | Explicit column names (implies no header in file). |

```bash
# 🔀 CSV → TSV
fimod s -i data.csv -e 'data' --output-format csv --csv-output-delimiter '\t'
```

!!! info "Column order is preserved"
    No alphabetical sorting through transforms.

When the input CSV has a header row, a `headers` global is automatically available in your script — see [Mold Scripting — CSV headers](mold-scripting.md#csv-headers-global).

---

## ⚡ Inline expressions vs scripts

=== "📝 Inline (`-e`)"

    Best for one-liners:

    ```bash
    fimod s -i users.json -e '[u for u in data if u["active"]]'
    fimod s -i data.json -e '{"name": data["first"].upper()}'
    fimod s -i data.json -e 're_findall(r"\d+", data["text"])'
    ```

    Multi-statement? Write `def transform` inside `-e`.

=== "📄 Script file (`-m`)"

    For reusable transforms:

    ```python
    # cleanup.py
    def transform(data, args, env, headers):
        for row in data:
            row["name"] = row["name"].strip().title()
        return data
    ```

    ```bash
    fimod s -i messy.csv -m cleanup.py -o clean.json
    ```

=== "🌐 Remote (`-m URL`)"

    Load from a URL:

    ```bash
    fimod s -i data.json -m https://example.com/transforms/normalize.py
    ```

---

## 🎛️ Special output formats

Sometimes you need to control the output format for chaining with other tools, without changing the file extension:

```bash
# 📦 Compact JSON: one-line JSON for piping
fimod s -i data.json -e 'data' --output-format json-compact
# {"name":"Alice","age":30}

# 🐚 TXT: evaluate string without JSON quotes (ideal for shell variables)
NAME=$(fimod s -i data.json -e 'data["name"]' --output-format txt)
echo "Hello $NAME"   # Hello Alice

# 📥 Raw: download binary streams or raw bytes (no parsing, bypass pipeline)
fimod shape -i https://example.com/file.bin --output-format raw -o file.bin
```

---

## 📎 Passing variables (`--arg`)

```bash
fimod s -i users.json --arg threshold=30 -e '
  [u for u in data if u["age"] > int(args["threshold"])]
'

# Or via mold:
fimod s -i data.json -m filter.py --arg threshold=30 --arg prefix="A"
```

Access via `args["key"]` in the `transform(data, args, env, headers)` function.

---

## 🔀 Format conversion

Pass-through expression for pure format conversion:

```bash
fimod s -i config.yaml -e 'data' -o config.toml
fimod s -i data.csv -e 'data' --output-format json
fimod s -i users.json -e 'data' --output-format lines
```

---

## 🐛 Debug mode (`--debug` / `-d`)

Prints pipeline diagnostics to **stderr** (stdout stays clean for piping):

```bash
fimod s -i data.json -m transform.py --debug
```

Output includes: input/output format, mold source, full script, input data, output data.

!!! tip
    In debug mode, Python `print()` statements are also redirected to stderr.

---

## 📢 Message verbosity (`--quiet` / `--msg-level`)

Control which `msg_*` functions in mold scripts produce output:

```bash
fimod s -i data.json -m transform.py --quiet              # errors only
fimod s -i data.json -m transform.py --msg-level=verbose  # + msg_verbose()
fimod s -i data.json -m transform.py --msg-level=trace    # + msg_trace()
```

`--quiet` and `--msg-level` are mutually exclusive. See [built-ins reference](../reference/built-ins.md#-message-functions-msg_) for the full visibility table.

---

## ✏️ In-place editing (`--in-place`)

Modify the input file directly:

```bash
fimod s -i config.json -e '{"host": data["host"], "port": data["port"]}' --in-place
```

Requires `-i`. Incompatible with `-o`.

---

## 🔌 Stdin / stdout

```bash
# 📥 Read from stdin
cat data.json | fimod s -e '{"count": len(data)}'

# 🔗 Pipe chain
curl -s https://jsonplaceholder.typicode.com/todos | fimod s -e '[d for d in data if d["completed"]]' | jq .
```

---

## 🐍 Monty REPL (`fimod monty repl`)

Start an interactive Python REPL powered by the embedded Monty engine — no system Python needed.

```bash
fimod monty repl
# Monty REPL v0.0.8 — fimod v0.1.0-alpha.1 (exit or Ctrl+D to quit)
# >>> 2 + 2
# 4
# >>> [x ** 2 for x in range(5)]
# [0, 1, 4, 9, 16]
# >>> exit
```

Features:

- **`>>>`/`...` prompts** with automatic multi-line continuation (blocks, implicit line joins)
- **Command history** (arrow keys, via rustyline)
- Exit with `exit` or ++ctrl+d++

!!! tip "Same engine as `fimod shape`"
    The REPL runs the exact same Monty runtime used by `fimod shape`, so you can experiment with Python expressions and data structures before putting them into a mold.

---

## 🐚 Shell completion

=== "Bash"

    ```bash
    fimod --completions bash > ~/.local/share/bash-completion/completions/fimod
    ```

=== "Zsh"

    ```bash
    fimod --completions zsh > ~/.zfunc/_fimod
    ```

=== "Fish"

    ```bash
    fimod --completions fish > ~/.config/fish/completions/fimod.fish
    ```
