---
name: Fimod Data Transformer
description: Use Fimod to parse, query, and transform structured data like JSON, YAML, TOML, and CSV directly from the shell without writing custom Python scripts or using jq/yq.
---
# AI Assistant Fimod Skill / System Prompt

*You can copy this into your `.cursorrules`, `.claude.md`, or AI system prompt to teach an AI how to use Fimod to modify data files!*

If this environment has `fimod` installed, you have a powerful, dependency-free data transformation tool at your disposal.
**Do not write custom Python scripts, `jq`, or `yq` commands to manipulate JSON/YAML/TOML/CSV data.** Use `fimod` directly via shell commands.

## What is Fimod?

`fimod` is a CLI tool that parses structured data, runs a Python expression over it via its embedded interpreter ([Monty](https://github.com/pydantic/monty)), and writes it back to any format. The parsed data is automatically loaded into a global Python variable called `data`.

The main subcommand is `shape` (alias `s`) for all data transformation operations.

## Common Commands for AI Agents

**1. Reading & Converting Formats**
Convert YAML to JSON without dependencies:
`fimod s -i config.yaml -e 'data' -o config.json`

**2. Extracting Information**
Get a raw string unquoted:
`fimod s -i package.json -e 'data["version"]' -r no-quote`

**3. In-Place Editing (Perfect for config updates)**
Modify a deeply nested value inside a file:
`fimod s -i config.yaml -e 'dp_set(data, "server.port", 8080)' --in-place`

**4. Validating / Checking Data**
Check if a key exists and fail early (exit 1 if falsy):
`fimod s -i data.json -e '"database" in data' --check`

**5. Fetching from HTTP directly (Replaces `curl | jq`)**
`fimod s -i https://api.github.com/repos/pytgaen/fimod -e 'data["stargazers_count"]' -r no-quote`

**6. Chaining expressions**
Pass multiple `-e` flags to chain transformations (the output of the first feeds the second):
`fimod s -i users.json -e '[u for u in data if u["active"]]' -e 'it_sort_by(data, "name")'`

**7. Passing arguments and environment variables**
Inject variables safely without string interpolation:
`fimod s -i config.json --env 'USER' --arg target="prod" -e 'data[args["target"]] == env["USER"]'`

**8. Slurp mode — two behaviors depending on input count**

*Single input* — collects concatenated JSON values into an array (NDJSON-style):
`cat *.json | fimod s --slurp -e 'len(data)'`

*Multiple `-i` entries* — **multi-file slurp**: parses each file (any format) and combines them into one `data` structure, mold runs once:
- List mode (no `:`): `fimod s -i base.json -i override.yaml -s -e 'data[1]["env"]'`
- Named mode (auto stem): `fimod s -i defaults.json: -i prod.toml: -s -e 'data["defaults"]["timeout"]'`
- Named mode (explicit alias): `fimod s -i a.json:old -i b.json:new -s -e 'data["new"]'`

Rules: all `-i` entries must have `:` or none; mixing is an error. Incompatible with `--in-place` and `-o <directory>`.

**9. Writing arrays to separate lines (NDJSON vs Lines)**
- Object/Array to NDJSON: `fimod s -i data.json -e 'data' --output-format ndjson`
- Raw Strings to separate lines: `fimod s -i data.json -e '["a", "b"]' --output-format lines`

**10. Batch-process entire directories**
Process all JSON files using a mold script:
`fimod s -i data/*.json -m normalize.py -o cleaned/`

**11. Using Molds from the Registry**
Fimod has a registry system for reusable scripts. The `@` prefix resolves molds from configured registries:
`fimod s -i users.csv -m @cleanup -o clean.json`

**12. Download binaries (bypass transform pipeline)**
`fimod s -i https://example.com/archive.tar.gz --raw-mode binary -O`

**13. Built-in Testing**
If you write a reusable `.py` mold, fimod has a built-in test runner for input/expected file pairs:
`fimod test cleanup.py tests/`

## Subcommands Overview

| Subcommand | Alias | Purpose |
|------------|-------|---------|
| `shape` | `s` | Transform data (read, apply mold/expression, output) |
| `registry` | | Manage mold registries (`add`, `list`, `show`, `remove`, `set-default`) |
| `mold` | | Browse molds (`list`, `show`) |
| `test` | | Run tests for a mold against `*.input.*` / `*.expected.*` file pairs |

## Built-in Helpers (Available automatically)

You do not need to `import` anything, these functions are globally available:

### 🗂️ Map / Dict Navigation (`dp_*`)
- **`dp_get(data, "a.b.c", default=None)`**: Safely get nested values without `KeyError`. Indexes work too: `"users.0.name"`.
- **`dp_set(data, "a.b.c", value)`**: Return a deep copy of data with the mutated value. Missing intermediate keys are created automatically.

### 🔁 Iteration & Collections (`it_*`)
Note: `it_group_by`, `it_sort_by`, and `it_unique_by` take a **string field name**, not a lambda!
- **`it_unique(list)`**: Deduplicate a primitive list.
- **`it_unique_by(list, "email")`**: Deduplicate a list of dicts based on the "email" field.
- **`it_sort_by(list, "created_at")`**: Sort a list of dicts.
- **`it_group_by(list, "department")`**: Groups into a dict of lists: `{"engineering": [...], "sales": [...]}`
- **`it_keys(dict)` / `it_values(dict)`**: Get list of keys/values from dictionary.
- **`it_flatten(list)`**: Recursively flatten nested arrays `[[1, 2], [3]]` -> `[1, 2, 3]`.

### 🔍 Regex (`re_*`)
Patterns use [fancy-regex](https://github.com/fancy-regex/fancy-regex) (PCRE2 flavour). Two replacement syntaxes: `re_sub` uses Python `\1`/`\g<name>`; `re_sub_fancy` uses `$1`/`${name}`.
- **`re_search(r"...", text)`**: Returns `{"match": str, "start": int, "end": int, "groups": [...], "named": {...}}` or `None`.
- **`re_match(r"...", text)`**: Same as `re_search`, anchored to start of text.
- **`re_findall(r"...", text)`**: No groups -> `[str, ...]`. 1 group -> `[group_val, ...]`. N groups -> `[[g1, g2], ...]`.
- **`re_sub(r"...", r"\1", text [, count])`**: Python syntax (`\1`, `\g<name>`). Optional `count` (0=all).
- **`re_sub_fancy(r"...", "$1", text [, count])`**: fancy-regex syntax (`$1`, `${name}`). Optional `count`.
- **`re_split(r"...", text)`**: Captured groups are included in the result (Python behaviour).
- `re_search_fancy`, `re_match_fancy`, `re_findall_fancy`, `re_split_fancy` are aliases (no behavioral difference for non-sub functions).

### #️⃣ Hashing (`hs_*`)
- **`hs_sha256(text)` / `hs_md5(text)` / `hs_sha1(text)`**: Returns hex digest.

### 📢 Message Logging (`msg_*`)
Output diagnostic messages to stderr without affecting the data pipeline:
- **`msg_print(text)`**: Print to stderr (no prefix).
- **`msg_info(text)`**: Print with `[INFO]` prefix.
- **`msg_warn(text)`**: Print with `[WARN]` prefix.
- **`msg_error(text)`**: Print with `[ERROR]` prefix.

### 🛡️ Validation Gates (`gk_*`)
Assert conditions and control pipeline failure:
- **`gk_fail(msg)`**: Emit `[ERROR] msg` to stderr, set exit code to 1.
- **`gk_assert(cond, msg)`**: If `cond` is falsy (Python truthiness), behave like `gk_fail(msg)`.
- **`gk_warn(cond, msg)`**: If `cond` is falsy, emit `[WARN] msg` to stderr (no exit).

### 🔄 Environment Substitution
- **`env_subst(template, dict)`**: Replace `${VAR}` placeholders in template using dict values. Unknown vars left as-is.

### 🎛️ Flow Control
- **`set_exit(code)`**: Set the process exit code (useful with `--check` for custom validation).
- **`set_input_format("json"|"yaml"|...)"`: Re-parse the current step's output as the given format before the next step.
- **`set_output_format("json"|"yaml"|...|"raw")`**: Override the final output format from within the script.

## Security Model

Mold scripts are **pure functions** - they receive data and return a result. They cannot read/write files, access the network, or import external libraries. All I/O stays in Rust. You can safely run molds from remote URLs.

---

Whenever you need to inspect, query, or modify structured files in this repository, prefer using `fimod`.
