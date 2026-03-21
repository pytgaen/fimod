<p align="center">
  <img src="docs/assets/logo-image.jpg" alt="fimod" width="380"/>
</p>

<h3 align="center">🏗️ Mold your data, shape your CI, play with your pipelines</h3>
<h3 align="center">🪶 Python-powered molding without Python installed</h3>

<h4 align="center">💡 DRY your pipelines · Slim your container images · Tame your configs</h4>

<p align="center">
  <a href="https://github.com/pytgaen/fimod/releases"><img src="https://img.shields.io/github/v/release/pytgaen/fimod?style=flat-square" alt="Release"></a>
  <a href="LICENSE.txt"><img src="https://img.shields.io/badge/license-LGPL--3.0-blue?style=flat-square" alt="License"></a>
  <a href="https://github.com/pytgaen/fimod/actions"><img src="https://img.shields.io/github/actions/workflow/status/pytgaen/fimod/release.yml?style=flat-square" alt="CI"></a>
</p>

---

**fimod** (**F**lexible **I**nput, **M**old **O**utput **D**ata) is a single Rust binary (~2.3 MB, UPX-compressed) with an embedded Python runtime ([Monty](https://github.com/pydantic/monty)). It reads **JSON, YAML, TOML, CSV, NDJSON, and plain text** - from files or directly from **HTTP URLs** - lets you transform data with Python expressions, and writes the result in any of those formats. No system Python, no `pip install`, no dependencies.

```bash
# 🔍 Filter, reshape, convert - in one command
fimod s -i users.json -e '[u for u in data if u["active"]]' -o active.csv
```

![Hero Demo](docs/assets/demo-hero.gif)

```bash
# ⛓️ Chain transforms like Unix pipes - inside a single process. Also have some built-in helpers.
fimod s -i data.json -e '[u for u in data if u["age"] > 30]' -e 'it_sort_by(data, "name")'

# 📦 Batch-process entire directories
fimod s -i logs/*.json -m normalize.py -o cleaned/
```

## 📦 Install

### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
```

The script downloads the right binary, installs it, then runs `fimod registry setup` to configure the official mold catalog.

> 💡 Options via env vars: `FIMOD_VARIANT=slim` · `FIMOD_INSTALL=~/.local/bin` · `FIMOD_VERSION=0.1.0`

### Windows

<details>
<summary><strong>Option 1 — Script PowerShell (two-step)</strong></summary>

The pipe-to-execute pattern triggers antivirus false positives. Download first, then run:

```powershell
Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
& "$env:TEMP\fimod-install.ps1"
```

> 💡 Same env var options as Linux: `$env:FIMOD_VARIANT`, `$env:FIMOD_INSTALL`, `$env:FIMOD_VERSION`

</details>

<details>
<summary><strong>Option 2 — via ubi (no script, antivirus-friendly)</strong></summary>

[ubi](https://github.com/houseabsolute/ubi) is a universal binary installer available on winget (pre-installed on Windows 10/11):

```powershell
# 1. Install ubi (one-time, uses winget which is built into Windows)
winget install houseabsolute.ubi

# 2. Install fimod
ubi --project pytgaen/fimod --in "$env:USERPROFILE\.local\bin"

# 3. Add to PATH (if not already present)
$BinDir = "$env:USERPROFILE\.local\bin"
$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($UserPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable('PATH', "$BinDir;$UserPath", 'User')
    $env:PATH = "$BinDir;$env:PATH"
}

# 4. Set up the official mold catalog
fimod registry setup
```

</details>

### From source

```bash
git clone https://github.com/pytgaen/fimod && cd fimod
cargo build --release   # → target/release/fimod
```

## 🤔 Why not jq / yq / awk / sed?

You already know Python. Why learn another DSL?

**jq / yq** - powerful but you need to learn a custom query language:

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

**Python one-liner** - works but painful boilerplate:

```bash
python3 -c "
import json, sys
data = json.load(sys.stdin)
print(json.dumps([u for u in data if u['active']]))
" < users.json

# fimod: same logic, zero boilerplate, no Python install
fimod s -i users.json -e '[u for u in data if u["active"]]'
```

👉 [**See the full feature comparison against jq, yq, and Python**](docs/guides/comparison.md)

## 🔋 Batteries included

### 🗂️ Multi-file slurp

The classic `yq`/`jq` slurp use case — merge a base config with environment overrides — but across **any mix of formats**:

```bash
# Merge base.yaml with prod overrides in TOML — impossible with yq
fimod s -i base.yaml -i prod.toml -s -e '
def transform(data):
    data[0].update(data[1])
    return data[0]
'
```

![Slurp Demo](docs/assets/demo-slurp.gif)

`data` is an array ordered like the `-i` flags; later entries win on conflict.

**Named mode** — append `:` to get a dict keyed by filename stem, clearer than an index when files have distinct roles:

```bash
# Merge base with prod overrides — role is explicit, no need to count -i flags
fimod s -i base.yaml: -i prod.yaml: -s -e '
def transform(data):
    data["base"].update(data["prod"])
    return data["base"]
'
```

**Explicit aliases** — when two files share the same name:

```bash
# Merge configs from sibling directories
fimod s -i eu/limits.toml:eu -i us/limits.toml:us -s \
  -e '{ region: v["max_requests"] for region, v in data.items() }'
```

The mold runs **once** on the combined result. Works across formats (JSON + YAML + TOML + CSV…).

### ⛓️ Chaining

Multiple `-e` expressions form an in-process pipeline - each step feeds `data` to the next:

```bash
fimod s -i data.json \
  -e '[u for u in data if u["age"] > 18]' \
  -e 'it_sort_by(data, "name")' \
  -e '[{"name": u["name"], "hash": hs_sha256(u["email"])} for u in data]'
```

![Chaining Demo](docs/assets/demo-chaining.gif)

### 🧰 Built-in helpers - no import needed

| Family | Functions | Example |
|--------|-----------|---------|
| `re_*` | search, match, findall, sub, split | `re_sub(r"(\w+)@(\w+)", r"\2/\1", text)` |
| `re_*_fancy` | same + fancy-regex `$1`/`${name}` syntax | `re_sub_fancy(r"(\w+)@(\w+)", "$2/$1", text)` |
| `dp_*` | get, set (nested dotpath) | `dp_set(data, "server.port", 8080)` |
| `it_*` | sort_by, group_by, unique, flatten, ... | `it_group_by(data, "status")` |
| `hs_*` | md5, sha1, sha256 | `hs_sha256(data["email"])` |
| `msg_*` | print, info, warn, error (to stderr) | `msg_warn("low coverage")` |
| `gk_*` | fail, assert, warn (validation gates) | `gk_assert(data.get("version"), "missing version")` |
| `env_subst` | `${VAR}` substitution in templates | `env_subst("Hello ${NAME}", env)` |

> Helpers are implemented in Rust. Regex patterns use [fancy-regex](https://github.com/fancy-regex/fancy-regex) (PCRE2). `re_sub` accepts Python `\1`/`\g<name>` syntax; `re_sub_fancy` uses `$1`/`${name}`.

### 📦 Reusable molds & registries

A **mold** is a Python file with a `transform(data, args, env, headers)` function. Inline `-e` expressions are great for one-liners, molds are for transforms you want to name, test, and share.

```python
# normalize.py
def transform(data, args, env, headers):
    return [{"name": u["name"].strip().title(), "email": u["email"].lower()} for u in data]
```

```bash
# Use a local mold
fimod s -i users.json -m normalize.py

# Use a remote mold - fetched and executed on the fly
fimod s -i users.json -m https://example.com/transforms/normalize.py
```

**Registries** are named collections of molds (local directories or GitHub/GitLab repos). The `@` prefix resolves molds from registries:

```bash
fimod registry add team https://github.com/myorg/molds --default
fimod s -i data.csv -m @clean_csv          # from default registry
fimod s -i data.csv -m @team/clean_csv     # explicit registry
fimod mold list                           # browse available molds
fimod mold show @clean_csv               # inspect metadata & defaults
```

<details>
<summary><strong>Private registry with token</strong></summary>

For private GitHub/GitLab repos, fimod automatically uses `$GITHUB_TOKEN` or `$GITLAB_TOKEN`:

```bash
# 1. Export your token (add to .bashrc/.zshrc for persistence)
export GITHUB_TOKEN=ghp_xxx

# 2. Add a private registry
fimod registry add corp https://github.com/myorg/private-molds --default

# 3. Use molds — token is picked up automatically
fimod s -i data.json -m @corp/sanitize

# Verify token is detected
fimod registry show corp
#   Token:   $GITHUB_TOKEN (auto) — set ✓
```

You can also use a custom env var per registry:

```bash
fimod registry add corp https://github.com/myorg/private-molds --token-env CORP_TOKEN
export CORP_TOKEN=ghp_yyy
```

</details>

**CI/ephemeral environments** — use `FIMOD_REGISTRY` instead of `fimod registry add`:

```bash
FIMOD_REGISTRY=./molds fimod s -i data.json -m @clean
FIMOD_REGISTRY="ci=./molds,staging=https://github.com/org/molds" fimod s -i data.json -m @ci/clean
```

fimod ships with a [built-in mold catalog](molds/README.md) covering common tasks (CSV stats, JSON schema extraction, key renaming, PII anonymization, and more).

## 🔥 HTTP input (goodbye `curl | jq`)

**The `-i` flag accepts URLs just like file paths.** No `curl`, no `wget`, no pipes. Fimod fetches, parses, and transforms in a single command.

```bash
# Fetch and transform in one shot - replaces curl | jq
fimod s -i https://api.github.com/repos/pytgaen/fimod -e 'data["name"] + ": " + str(data["stargazers_count"]) + " stars"' --output-format txt

# Hit authenticated APIs with custom headers
fimod s -i https://api.github.com/user/repos \
    --http-header "Authorization: Bearer $GITHUB_TOKEN" \
    -e '[r["full_name"] for r in data]'

# 👀 Download binaries - bypass the transform pipeline entirely
fimod s -i https://example.com/archive.tar.gz --output-format raw -O
```

![HTTP Demo](docs/assets/demo-http.gif)

Powered by [reqwest](https://github.com/seanmonstar/reqwest) with rustls - proxy-aware out of the box (`HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY`). Smart format detection reads `Content-Type` headers automatically. Use `--input-format http` for full access to status codes and response headers.

> Requires the `full` build variant (default). Use `FIMOD_VARIANT=slim` to exclude HTTP support.

## 🛡️ Security model

Mold scripts are **pure functions** - they receive data and return a result. They cannot:

- Read/write files, access the network, or call the OS
- Import external libraries

All I/O stays in Rust. You can safely run molds from remote URLs without sandboxing concerns.

## ⚙️ How it works

```
 Input                  Python transform           Output
┌──────────────┐       ┌───────────────────┐      ┌──────────────┐
│ file / stdin │       │                   │      │ JSON / YAML  │
│ https://...  │─────▶│  your transform   │─────▶│ TOML / CSV   │
│ JSON / YAML  │ Rust  │  runs in Monty    │ Rust │ NDJSON / TXT │
│ TOML / CSV   │ parse │  (embedded Python)│ ser. └──────────────┘
│ NDJSON / TXT │       └───────────────────┘
└──────────────┘
```

## 📖 Documentation

| 📚 Guides | 🔧 Reference |
|---|---|
| [Quick Start](docs/guides/quick-start.md) | [Formats](docs/reference/formats.md) - JSON, YAML, TOML, CSV, TXT, Lines, NDJSON, HTTP |
| [Concepts](docs/guides/concepts.md) | [Built-ins](docs/reference/built-ins.md) - `re_*`, `dp_*`, `it_*`, `hs_*`, `msg_*`, `gk_*`, `env_subst` |
| [Mold Scripting](docs/guides/mold-scripting.md) | [Mold Defaults](docs/reference/mold-defaults.md) - `# fimod:` directives |
| [CLI Reference](docs/guides/cli-reference.md) | [Exit Codes](docs/reference/exit-codes.md) - `--check` and `set_exit()` |
| [Authoring Molds](docs/guides/authoring-molds.md) | [Cookbook](docs/cookbook.md) 🍳 |
| [AI Integration & Agents](docs/guides/ai-integration.md) 🤖 | [Agent Skill](.agents/skills/fimod/SKILL.md) ✨ |

## ⚠️ Project Status

**fimod is young software - built with AI-assisted development ("vibe coding").**

- **Monty** (the embedded Python runtime) is an early-stage project by Pydantic. Its API is unstable and may change between releases.
- **fimod** depends directly on Monty and inherits that instability. Expect breaking changes as both projects mature.
- Versioning follows [Semantic Release](https://semver.org/) - breaking changes bump the major version.
- Built-in helpers (`re_*`, `dp_*`, `it_*`, `hs_*`, `msg_*`, `gk_*`, `env_subst`) are implemented in **Rust** to complement Monty's limited stdlib. In particular, regex functions use [fancy-regex](https://github.com/fancy-regex/fancy-regex) syntax (Rust/PCRE2 flavour), **not** Python's `re` module - see [Built-ins Reference](docs/reference/built-ins.md).

> [!NOTE]
> **Regex: Fimod built-ins vs Monty's `re` module**
>
> Fimod was originally built on Monty v0.0.6, which had no regex support.
> We introduced `re_search`, `re_sub`, `re_findall`, etc. as Fimod built-in functions to fill that gap — a good example of the challenges of moving fast alongside a young runtime.
>
> Since Monty v0.0.8, `import re` works — Monty implements a subset of Python's `re` module.
> Both approaches now work side by side:
>
> - **Fimod's `re_*` built-ins** — direct access to [fancy-regex](https://github.com/fancy-regex/fancy-regex), including advanced features like variable-length lookbehind/lookahead
> - **`import re`** — familiar Python API, but only [partially implemented in Monty](https://github.com/pydantic/monty/pull/157) (also backed by fancy-regex under the hood)
>
> The `re_*` built-ins are here to stay for the foreseeable future (at least until late 2027). As Monty's `re` module matures, we'll reconsider.
>
> Since `import re` is already well-known to Python developers, the documentation focuses on the `re_*` built-ins which are specific to Fimod.

## 📄 License

GNU Lesser General Public License v3.0 - see [LICENSE.txt](LICENSE.txt).
