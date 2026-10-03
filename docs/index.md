<p align="center">
  <img src="assets/logo-image.jpg" alt="fimod" width="380"/>
</p>

# 🐍🦀 fimod — *the data shaper CLI*

> **🏗️ Mold your data, shape your CI, play with your pipelines**
> **🪶 Python-powered molding without Python installed.**
>
> *💡 DRY your pipelines · Slim your container images · Tame your configs*

**fimod** (**F**lexible **I**nput, **M**old **O**utput **D**ata) embeds [Pydantic Monty](https://github.com/pydantic/monty) (a Rust implementation of Python) in a single binary (~3.3 MB in the UPX-compressed standard build). You write the transform logic; fimod handles parsing, format detection, and I/O.

!!! info "Python syntax, Monty runtime"
    Fimod runs molds on Monty, not CPython: use familiar Python syntax and built-ins, with Rust-powered helpers for data shaping.

```bash
# 🎯 One-liner
fimod s -i data.json -e '[u for u in data if u["active"]]'

# 📜 Reusable script
fimod s -i input.csv -m cleanup.py -o output.json

# 🔀 Format conversion
fimod s -i config.yaml -e 'data' -o config.toml

# 🔥 Fetch from URLs — no curl, no wget, no pipes!
fimod s -i https://api.github.com/repos/pytgaen/fimod -e 'data["name"]' --output-format txt
```

---

## ✨ Features

<div class="grid cards" markdown>

-   :material-language-python:{ .lg .middle } **Python you already know**

    ---

    No new DSL. Write `for`, `if`, comprehensions, and string methods using Monty's Python subset.

-   :material-package-variant-closed:{ .lg .middle } **Single binary**

    ---

    No runtime, no `pip install`, no dependencies. One ~3.3 MB standard binary after UPX compression; use `slim` when you do not need HTTP input, or `fimod-fast` for large CPU-heavy runs.

-   :material-swap-horizontal:{ .lg .middle } **All the formats**

    ---

    JSON · NDJSON · YAML · TOML · CSV · TXT · Lines — auto-detected from extension.

-   :material-toolbox-outline:{ .lg .middle } **Batteries included**

    ---

    Native `import re` for regex; `dp_*` dotpath · active `it_*` iteration · `hs_*` hashing · `msg_*` logging · `gk_*` validation · `env_subst` helpers require no imports. Deprecated helpers need [legacy activation](reference/built-ins.md#legacy-built-ins).

-   :material-web:{ .lg .middle } **🚀 Awesome 🔥 Your input can be an HTTPS request!**

    ---

    Awesome: `-i https://...` just works — fimod fetches (via `reqwest` with proxy and HTTPS support), parses, and transforms in one shot. Goodbye `curl | jq`!

</div>

---

## 👀 A taste of what fimod can do

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

---

## 🗺️ Guides

Start here if you're new to fimod.

<div class="grid cards" markdown>

-   :material-rocket-launch:{ .lg .middle } **Quick Start**

    ---

    Install fimod and run your first transform in 2 minutes.

    [:octicons-arrow-right-24: Get started](guides/quick-start.md)

-   :material-lightbulb-on-outline:{ .lg .middle } **Concepts**

    ---

    The pipeline, Monty, molds, and the security model — how it all fits together.

    [:octicons-arrow-right-24: Understand fimod](guides/concepts.md)

-   :material-language-python:{ .lg .middle } **Mold Scripting**

    ---

    Write transforms with native `import re`, dotpath, iteration, and hash helpers.

    [:octicons-arrow-right-24: Write molds](guides/mold-scripting.md)

-   :material-console-line:{ .lg .middle } **CLI Reference**

    ---

    All options and modes — slurp, check, no-input, in-place, args, debug, and more.

    [:octicons-arrow-right-24: Explore the CLI](guides/cli-reference.md)

</div>

---

## 📚 Reference

Lookup tables and complete specifications.

<div class="grid cards" markdown>

-   :material-file-multiple-outline:{ .lg .middle } **Formats**

    ---

    JSON, NDJSON, YAML, TOML, CSV, TXT, Lines — behavior and options for each.

    [:octicons-arrow-right-24: Formats](reference/formats.md)

-   :material-function-variant:{ .lg .middle } **Built-ins**

    ---

    Active helper signatures, legacy activation and migration, pipeline controls, `args`, and `headers`.

    [:octicons-arrow-right-24: Built-ins](reference/built-ins.md)

-   :material-tune-variant:{ .lg .middle } **Mold Defaults**

    ---

    `# fimod:` directives — embed format and option defaults directly in scripts.

    [:octicons-arrow-right-24: Mold Defaults](reference/mold-defaults.md)

-   :material-flag-checkered:{ .lg .middle } **Exit Codes**

    ---

    `--check` truthiness table and `set_exit` behavior explained.

    [:octicons-arrow-right-24: Exit Codes](reference/exit-codes.md)

</div>

---

## 🍳 Cookbook

[:material-chef-hat: Practical recipes](cookbook.md) — filtering, aggregation, regex, format conversion, validation, data generation, slurp, and more.

---

## ⚠️ Project Status

!!! warning "Early-stage software"
    fimod is young software, built with AI-assisted development ("vibe coding").

    - **[Monty](https://github.com/pydantic/monty)** is Pydantic's Rust implementation of a Python subset. Fimod pins Monty 1.0.0; this is not CPython or full standard-library parity.
    - **fimod** is still pre-1.0. Supported mold APIs may evolve as the project matures.
    - Versioning follows [Semantic Versioning](https://semver.org/). Before 1.0, breaking changes bump the minor version; after 1.0, they bump the major version.
    - Mold scripts can use Python syntax, common built-ins, and selected stdlib modules, but not arbitrary PyPI packages or full stdlib parity.
    - Fimod adds Rust helpers for dot paths, iteration, hashing, templating, logging, and validation. Legacy regex and selected iteration helpers require explicit activation; new regex molds use `import re`.

!!! note "Legacy built-ins"
    New molds use Monty's `import re` and Python deduplication/flattening code.
    Existing molds can keep `re_*`, `it_unique`, `it_unique_by`, and `it_flatten`
    by setting `FIMOD_LEGACY_BUILTINS=1`. No warning is emitted when enabled.
    Without activation, calling one raises an error with migration guidance.
    See the [migration guide](reference/built-ins.md#legacy-built-ins) for API differences.
