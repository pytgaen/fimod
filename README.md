<p align="center">
  <img src="docs/assets/logo-image.jpg" alt="fimod" width="380"/>
</p>

<h2 align="center"><em>the data shaper CLI</em></h2>

<h3 align="center">🏗️ Mold your data, shape your CI, play with your pipelines</h3>
<h3 align="center">🪶 Python-powered molding without Python installed</h3>

<h4 align="center">💡 DRY your pipelines · Slim your container images · Tame your configs</h4>

<p align="center">
  <a href="https://github.com/pytgaen/fimod/releases"><img src="https://img.shields.io/github/v/release/pytgaen/fimod?style=flat-square" alt="Release"></a>
  <a href="LICENSE.txt"><img src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square" alt="License"></a>
  <a href="https://github.com/pytgaen/fimod/actions"><img src="https://img.shields.io/github/actions/workflow/status/pytgaen/fimod/release.yml?style=flat-square" alt="CI"></a>
</p>

---

**fimod** (**F**lexible **I**nput, **M**old **O**utput **D**ata) is a single Rust binary (~3.3 MB in the UPX-compressed standard build) with an embedded Python runtime ([Monty](https://github.com/pydantic/monty)). It reads **JSON, YAML, TOML, CSV, NDJSON, and plain text** - from files or directly from **HTTP URLs** - lets you transform data with Python expressions, and writes the result in any of those formats. No system Python, no `pip install`, no dependencies.

Fimod runs molds on Monty, not CPython: use familiar Python syntax and built-ins, with Rust-powered helpers for data shaping.

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

Pre-built macOS archives are available for Apple Silicon (`arm64`) only. On an Intel Mac, use the [source build](#from-source).

The script downloads the right binary, installs it, then runs `fimod setup all defaults --if-needed` for **community registries** (example molds) and the **recommended sandbox policy** (`~/.config/fimod/sandbox.toml`). Already-configured blocks are skipped; missing blocks ask unless you answer with env vars. When installing `slim` or `fast`, the script may also ask whether to install that variant as the default `fimod` command.

Before extraction, the installer requires the release checksum manifest and an exact entry for the selected asset. Missing manifests, missing entries, and SHA-256 mismatches stop installation. The optional GitLab mirror may lag behind GitHub: `FIMOD_SOURCE=gitlab` therefore also requires an explicit `FIMOD_VERSION`, and that package must contain the selected asset and checksum manifest.

> 💡 Options via env vars: `FIMOD_VARIANT=standard|slim|fast` · `FIMOD_SET_DEFAULT=yes|no` · `FIMOD_INSTALL=~/.local/bin` · `FIMOD_VERSION=X.Y.Z` · `FIMOD_SOURCE=github|gitlab` · `FIMOD_SETUP_ALL=yes|no` (or per category: `FIMOD_SETUP_REGISTRY` / `FIMOD_SETUP_SANDBOX`)
>
> With `curl | sh`, put env vars on the `sh` side of the pipe: `curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | FIMOD_VARIANT=fast FIMOD_SET_DEFAULT=yes FIMOD_SETUP_ALL=yes sh`. Prefixing `curl` configures `curl`, not the installer.
>
> Variants install as separate commands by default: `standard` → `fimod`, `slim` → `fimod-slim`, `fast` → `fimod-fast`. For `slim` or `fast`, answer the prompt or set `FIMOD_SET_DEFAULT=yes` to also make that variant the default `fimod` command.
> `FIMOD_SET_DEFAULT` does not answer the registry or sandbox setup prompts; use `FIMOD_SETUP_ALL=yes` or `FIMOD_SETUP_REGISTRY` / `FIMOD_SETUP_SANDBOX` for post-install setup.
>
> The standard build includes HTTP/HTTPS and proxy support through `reqwest` + `rustls` + AWS-LC. Use `FIMOD_VARIANT=slim` when binary size matters more than HTTP input or remote mold loading.
>
> The `fast` build keeps the standard feature set but uses the speed profile (`opt-level=3`) and ships without UPX compression. Internal smoke tests show roughly 15-25% faster CPU-heavy JSON/CSV/chain workloads; use it for large files, long chains, or repeated batch jobs.

### Windows

<details>
<summary><strong>Option 1 — via ubi (no script, antivirus-friendly)</strong></summary>

[ubi](https://github.com/houseabsolute/ubi) is a universal binary installer available on winget (pre-installed on Windows 10/11):

```powershell
# 📦 1. Install ubi (one-time, uses winget which is built into Windows)
winget install houseabsolute.ubi

# 🔄 Then restart PowerShell so ubi is found in PATH

# ⬇️ 2. Install fimod (classic — includes HTTP support)
ubi --project pytgaen/fimod --matching "fimod-v" --in "$env:USERPROFILE\.local\bin"

# Or install the slim variant (no HTTP support, smaller binary)
# ubi --project pytgaen/fimod --matching "fimod-slim-v" --in "$env:USERPROFILE\.local\bin"

# Or install the fast variant (speed optimized, larger uncompressed binary)
# ubi --project pytgaen/fimod --matching "fimod-fast-v" --in "$env:USERPROFILE\.local\bin"

# 🛤️ 3. Add to PATH (if not already present)
$BinDir = "$env:USERPROFILE\.local\bin"
$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($UserPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable('PATH', "$BinDir;$UserPath", 'User')
    $env:PATH = "$BinDir;$env:PATH"
}

# 🗂️ 4. Install missing registries + sandbox policy
fimod setup all defaults --if-needed
```

</details>

<details>
<summary><strong>Option 2 — PowerShell script (execution policy / antivirus may block)</strong></summary>

> ⚠️ If your antivirus blocks this script, use **Option 1 (ubi)** instead — it downloads a prebuilt binary directly from GitHub Releases with no installer-script execution.

Download first, then run:

```powershell
Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
& "$env:TEMP\fimod-install.ps1"
```

The PowerShell installer applies the same mandatory checksum-manifest and exact-asset verification before extraction.

> 💡 Same env var options as Linux: `$env:FIMOD_VARIANT`, `$env:FIMOD_SET_DEFAULT`, `$env:FIMOD_INSTALL`, `$env:FIMOD_VERSION`, `$env:FIMOD_SOURCE`

</details>

<details>
<summary><strong>⚠️ VCRUNTIME140.dll not found?</strong></summary>

fimod requires the **Microsoft Visual C++ Redistributable**, pre-installed on most Windows systems but missing in minimal environments (Windows Sandbox, fresh server installs).

```powershell
winget install Microsoft.VCRedist.2015+.x64
```

Or download directly from Microsoft: https://aka.ms/vs/17/release/vc_redist.x64.exe

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

# fimod: same idea, with Python syntax
fimod s -i users.json -e '[u for u in data if u["age"] > 30]'
```

```bash
# jq: project + sort + deduplicate
jq '[.[] | {id, name}] | sort_by(.name) | unique_by(.id)' data.json

# fimod: chain expressions, each feeds the next
fimod s -i data.json -e '[{"id": u["id"], "name": u["name"]} for u in data]' \
  -e 'it_sort_by(data, "name")' -m @dedup_by --arg field=id
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

### 👀 A taste of what fimod can do

🐍 **Python-syntax transforms — Rust-powered I/O, serialization & builtins:**

```bash
# YAML to JSON, filter active users, sort by name
fimod s -i users.yaml -e '[u for u in data if u["active"]]' -e 'it_sort_by(data, "name")' -o result.json
```

```bash
# Filter active users, then group by role — Unix pipes just work
fimod s -i users.json -e '[u for u in data if u["active"]]' | fimod s -e 'it_group_by(data, "role")'
```

```bash
# Enrich records with Python string methods — try this in jq...
fimod s -i users.json -e '[{**u, "slug": u["name"].lower().replace(" ", "-"), "domain": u["email"].split("@")[1]} for u in data]'
```

📦 **Registry molds — reusable recipes, one `@name` away:**

```bash
# 🔀 Patch a YAML config with dot-path assignments
fimod s -i deployment.yaml -m @yaml_merge --arg set="spec.replicas=3,metadata.labels.env=prod" -o deployment.yaml
```

```bash
# 🔐 Anonymize PII fields with SHA-256
fimod s -i users.json -m @anonymize_pii --arg fields=email,phone -o users_anon.json
```

```bash
# 📊 Deduplicate records by a field
fimod s -i data.json -m @dedup_by --arg field=email
```

📦 **More molds** in the [fimod-powered](https://github.com/pytgaen/fimod-powered) registry:

| Mold | Description |
|------|-------------|
| `@gh_latest` | GitHub release resolver |
| `@download` | wget-like fetch |
| `@poetry_migrate` | Poetry → uv/Poetry 2 |
| `@skylos_to_gitlab` | dead code → GitLab Code Quality |

```bash
fimod registry add fimod-powered https://github.com/pytgaen/fimod-powered
```

<details>
<summary><strong>🍿 Even more taste... (in-place, regex, log parsing, env templating)</strong></summary>

```bash
# 🔒 Anonymize emails in-place — replace with SHA-256 hashes
fimod s -i customers.csv -e '[{**r, "email": hs_sha256(r["email"])} for r in data]' --in-place
```

```bash
# 🕵️ Mask IPs with regex — 192.168.1.42 → 192.168.x.x
fimod s -i logs.json -e '
import re
def transform(data, **_):
    return [{**row, "ip": re.sub(r"\d+\.\d+$", "x.x", row["ip"])} for row in data]
'
```

```bash
# 📊 Raw log lines → structured JSON records
fimod s -i server.log -m @log_parse \
  --arg regex='(\S+) \[(.+?)\] "(.+?)" (\d+)' \
  --arg fields=ip,timestamp,request,status
```

```bash
# 🔀 Inject environment variables into ${VAR} placeholders
fimod s -i config.json --env 'DB_*' -e '{k: env_subst(v, env) for k, v in data.items()}'
```

</details>

Run `fimod mold list` to browse all built-in molds.

## 🔋 Batteries included

### 🗂️ Multi-file slurp

The classic `yq`/`jq` slurp use case — merge a base config with environment overrides — but across **any mix of formats**:

```bash
# Merge base.yaml with prod overrides in TOML — impossible with yq
fimod s -i base.yaml -i prod.toml -s -e '
def transform(data, **_):
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
def transform(data, **_):
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
| `dp_*` | get, set (nested dotpath) | `dp_set(data, "server.port", 8080)` |
| `it_*` | keys, values, sort_by, group_by, count_by, min_by, max_by | `it_group_by(data, "status")` |
| `hs_*` | md5, sha1, sha256 | `hs_sha256(data["email"])` |
| `msg_*` | print, info, warn, error (to stderr) | `msg_warn("low coverage")` |
| `gk_*` | fail, assert, warn (validation gates) | `gk_assert(data.get("version"), "missing version")` |
| `env_subst` | `${VAR}` substitution in templates | `env_subst("Hello ${NAME}", env)` |

> Helpers are implemented in Rust. Use `import re` for regex. Legacy `re_*`, `it_unique`, `it_unique_by`, and `it_flatten` require `FIMOD_LEGACY_BUILTINS=1`, silently. See the [migration guide](docs/reference/built-ins.md#legacy-built-ins).

### 📦 Reusable molds & registries

A **mold** is a Python file with a `transform(data, **_)` function. Add named context parameters before `**_` when needed, for example `args` or `pipeline`. Keeping `**_` is the recommended convention for reusable molds because fimod passes context as keyword arguments.

```python
# normalize.py
def transform(data, **_):
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

Powered by [reqwest](https://github.com/seanmonstar/reqwest) with rustls/AWS-LC - proxy-aware out of the box (`HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY`). Smart format detection reads `Content-Type` headers automatically. Use `--input-format http` for full access to status codes and response headers.

> Included in the `standard` and `fast` variants. Use `FIMOD_VARIANT=slim` to exclude HTTP support.

## 🛡️ Security model

Mold scripts and Monty REPL snippets run under a **zero-authorization sandbox** by default. All I/O stays in Rust — a mold cannot read/write files, reach the network, or inspect the host process. Every `fimod s` invocation also enforces hard limits (`max_duration = 10m`, `max_memory = 2GB`) and exits with code `137` on violation. REPL violations are printed as errors and the session continues. Remote molds therefore start without host capabilities; review them before granting permissions. Resource limits are local guardrails, not a hostile-code or multi-tenant isolation boundary.

Opt in to the bits your molds need by writing `~/.config/fimod/sandbox.toml`:

```toml
[sandbox]
allow_clock  = true              # enable datetime.now() / date.today()
max_duration = "10m"
max_memory   = "2GB"
allow_env    = ["LANG", "TZ_*"]  # glob-matched os.getenv() keys
```

Bootstrap it with `fimod setup sandbox defaults --yes`, then tune it with `fimod setup sandbox set --max-duration 20m --max-memory 4GB` or inspect it with `fimod setup sandbox show`. Override a shape or REPL run with `--sandbox-file <path>`; force zero-authorization with `--sandbox-file=""`. See [Sandbox policy](docs/guides/cli-reference.md#sandbox-policy) for the full reference.

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

> 🧠 **Designed by humans — built by AI.**

**fimod is young software.** AI accelerates the velocity; humans own the architecture.

Design decisions, invariants, and architectural boundaries stay explicit — see [`notes/`](notes/) for the vision, architecture map, and design log. The discipline you see in the code (`cargo deny`, `#[must_use]`, layered serde/Monty boundary, conventional commits, ~500 tests + e2e fixtures) is intentional; the speed is the AI.

- **Monty** is Pydantic's embedded Python runtime. Fimod pins Monty 1.0.0; it is a Python subset, not full CPython.
- **fimod** is still pre-1.0. Supported mold APIs may evolve as the project matures.
- Versioning follows [Semantic Versioning](https://semver.org/). Before 1.0, breaking changes bump the minor version; after 1.0, they bump the major version.
- Mold scripts can use Python syntax, common built-ins, and selected stdlib modules, but not arbitrary PyPI packages or full stdlib parity.
- Fimod adds Rust helpers for dot paths, iteration, hashing, templating, logging, and validation. Legacy regex and selected iteration helpers require explicit activation; new regex molds use `import re`.

> [!NOTE]
> **Legacy built-ins**
>
> New molds use Monty's `import re` and Python deduplication/flattening code.
> Existing molds can keep `re_*`, `it_unique`, `it_unique_by`, and `it_flatten`
> by setting `FIMOD_LEGACY_BUILTINS=1`. No warning is emitted when enabled.
> Without activation, calling one raises an error with migration guidance.
> See the [migration guide](docs/reference/built-ins.md#legacy-built-ins) for API differences.

## 📄 License

Apache License 2.0 - see [LICENSE.txt](LICENSE.txt).
