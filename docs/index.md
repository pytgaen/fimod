<p align="center">
  <img src="assets/logo-image.jpg" alt="fimod" width="380"/>
</p>

# 🐍🦀 fimod

> **🏗️ Mold your data, shape your CI, play with your pipelines**
> **🪶 Python-powered molding without Python installed.**
>
> *💡 DRY your pipelines · Slim your container images · Tame your configs*

**fimod** (**F**lexible **I**nput, **M**old **O**utput **D**ata) embeds [Pydantic Monty](https://github.com/pydantic/monty) (a Rust implementation of Python) in a single binary (~2.3 MB, UPX-compressed). You write the transform logic; fimod handles parsing, format detection, and I/O.

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

    No new DSL. Write `for`, `if`, comprehensions, string methods — it's just Python.

-   :material-package-variant-closed:{ .lg .middle } **Single binary**

    ---

    No runtime, no `pip install`, no dependencies. One ~2.3 MB binary (UPX-compressed) that works everywhere.

-   :material-swap-horizontal:{ .lg .middle } **All the formats**

    ---

    JSON · NDJSON · YAML · TOML · CSV · TXT · Lines — auto-detected from extension.

-   :material-toolbox-outline:{ .lg .middle } **Batteries included**

    ---

    `re_*` regex · `dp_*` dotpath · `it_*` iteration · `hs_*` hashing · `msg_*` logging · `gk_*` validation · `env_subst` — no imports needed.

-   :material-web:{ .lg .middle } **🚀 Awesome 🔥 Your input can be an HTTPS request!**

    ---

    Awesome: `-i https://...` just works — fimod fetches (via `reqwest` with proxy and HTTPS support), parses, and transforms in one shot. Goodbye `curl | jq`!

</div>

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

    Write transforms with built-in regex, dotpath, iteration, and hash helpers.

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

    Complete signatures for `re_*`, `dp_*`, `it_*`, `hs_*`, `msg_*`, `gk_*`, `env_subst`, `set_exit`, `set_input_format`, `set_output_format`, `set_output_file`, `args`, `headers`.

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

    - **[Monty](https://github.com/pydantic/monty)** is an early-stage Rust implementation of Python by Pydantic. Its API is unstable and may introduce breaking changes.
    - **fimod** depends directly on Monty and inherits that instability. Expect breaking changes as both projects mature.
    - Versioning follows [Semantic Versioning](https://semver.org/) — breaking changes bump the major version.
    - Built-in helpers (`re_*`, `dp_*`, `it_*`, `hs_*`, `msg_*`, `gk_*`, `env_subst`) are implemented in **Rust** to compensate for Monty's lack of a standard library. Regex in particular uses [fancy-regex](https://github.com/fancy-regex/fancy-regex) syntax (Rust/PCRE2), **not** Python's `re` module — see [Built-ins → Regex](reference/built-ins.md#regex-functions-re).
