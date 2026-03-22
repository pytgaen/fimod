---
name: Fimod Data Transformer
description: Use Fimod to parse, query, and transform structured data like JSON, YAML, TOML, CSV, and plain text directly from the shell without writing custom Python scripts or using jq/yq.
---
# AI Assistant Fimod Skill / System Prompt

*You can copy this into your `.cursorrules`, `.claude.md`, or AI system prompt to teach an AI how to use Fimod to modify data files!*

If this environment has `fimod` installed, you have a powerful, dependency-free data transformation tool at your disposal.
**Do not write custom Python scripts, `jq`, or `yq` commands to manipulate JSON/YAML/TOML/CSV data.** Use `fimod` directly via shell commands.

## What is Fimod?

`fimod` is a single-binary CLI tool (~2 MB) that parses structured data, runs a Python expression over it via its embedded interpreter ([Monty](https://github.com/pydantic/monty)), and writes it back to any format. No system Python installation required. The parsed data is automatically loaded into a global Python variable called `data`.

The main subcommand is `shape` (alias `s`) for all data transformation operations.

## Common Commands for AI Agents

**1. Reading & Converting Formats**
Convert YAML to JSON without dependencies:
`fimod s -i config.yaml -e 'data' -o config.json`

**2. Extracting Information**
Get a raw string unquoted:
`fimod s -i package.json -e 'data["version"]' --output-format txt`

**3. In-Place Editing (Perfect for config updates)**
Modify a deeply nested value inside a file:
`fimod s -i config.yaml -e 'dp_set(data, "server.port", 8080)' --in-place`

**4. Validating / Checking Data**
Check if a key exists and fail early (exit 1 if falsy):
`fimod s -i data.json -e '"database" in data' --check`

**5. Fetching from HTTP directly (Replaces `curl | jq`)**
`fimod s -i https://api.github.com/repos/pytgaen/fimod -e 'data["stargazers_count"]' --output-format txt`

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

Set up the official registry first:
`fimod registry setup`

**12. Download binaries (bypass transform pipeline)**
`fimod s -i https://example.com/archive.tar.gz --output-format raw -O`

**13. Built-in Testing**
If you write a reusable `.py` mold, fimod has a built-in test runner for input/expected file pairs:
`fimod test cleanup.py tests/`

**14. No-input mode — generate data from scratch**
`fimod s --no-input -e '{"status": "ok", "ts": args["ts"]}' --arg ts="2024-01-01"`

**15. Input list from file or stdin**
Read input paths line-by-line from a file or stdin:
`find . -name '*.json' | fimod s -I - -e 'len(data)' -o results/`

**16. HTTP with custom headers and raw response**
```bash
# Authenticated API request
fimod s -i https://api.github.com/user \
    --http-header "Authorization: Bearer $GITHUB_TOKEN" \
    -e 'data["login"]' --output-format txt

# Raw HTTP response (status, headers, body)
fimod s -i https://example.com/api --input-format http \
    -e '{"status": data["status"], "type": data["headers"]["content-type"]}'
```

**17. Fetch + Re-parse with `set_input_format()`**
```bash
fimod s -i https://api.github.com/repos/pytgaen/fimod/releases/latest \
    --input-format http \
    -e 'set_input_format("json"); data["body"]' \
    -e '{"tag": data["tag_name"], "date": data["published_at"]}'
```

Or with `cast_input_format()` for one-liners:
```bash
fimod s -i https://jsonplaceholder.typicode.com/todos/1 \
    --input-format http \
    -e 'cast_input_format("json", data["body"])' \
    -e 'data["title"]' --output-format txt
```

**18. Log analysis with `--input-format lines`**
```bash
# Filter error lines
fimod s -i app.log --input-format lines \
  -e '[l for l in data if "ERROR" in l]'

# Filter with regex
fimod s -i access.log --input-format lines \
  -e '[l for l in data if re_search(r"\s[45]\d{2}\s", l)]'
```

**19. Nested API response to flat CSV**
```bash
fimod s -i https://jsonplaceholder.typicode.com/users \
  -e '[{
    "id": u["id"],
    "name": u["name"],
    "email": u["email"],
    "city": dp_get(u, "address.city"),
    "company": dp_get(u, "company.name")
  } for u in data]' \
  -o contacts.csv
```

**20. Monty REPL — interactive Python**
`fimod monty repl` — starts an interactive Python REPL powered by the same embedded Monty engine. Useful for experimenting with expressions before putting them into a mold.

## Subcommands Overview

| Subcommand | Alias | Purpose |
|------------|-------|---------|
| `shape` | `s` | Transform data (read, apply mold/expression, output) |
| `registry` | | Manage mold registries (`setup`, `add`, `list`, `show`, `remove`, `set-default`, `build-catalog`) |
| `mold` | | Browse molds (`list`, `show`) |
| `test` | | Run tests for a mold against `*.input.*` / `*.expected.*` file pairs |
| `monty repl` | | Interactive Python REPL (embedded Monty engine) |

## CLI Flag Reference

### Core Options
| Option | Short | Description |
|--------|-------|-------------|
| `--input` | `-i` | Input file path or URL (repeatable for batch/slurp) |
| `--input-list` | `-I` | Read input paths from a file or stdin (`-`) |
| `--mold` | `-m` | Path, URL, or `@name` of the transform script |
| `--expression` | `-e` | Inline Python expression (repeatable for chaining; mutually exclusive with `-m`) |
| `--output` | `-o` | Output file path or directory |
| `--url-filename` | `-O` | Use filename from URL (like `curl -O`) |
| `--in-place` | | Modify input file in-place |
| `--slurp` | `-s` | Read multiple JSON values into array / multi-file slurp |
| `--no-input` | | Skip input — `data = None` |
| `--check` | | No stdout; exit 0 if truthy, 1 if falsy |
| `--debug` | `-d` | Print pipeline diagnostics to stderr |

### Format Control
| Option | Description |
|--------|-------------|
| `--input-format` | Explicitly set input format (auto-detected from extension if omitted) |
| `--output-format` | Explicitly set output format (defaults to input format) |

Supported: `json` · `json-compact` · `ndjson` · `yaml` · `toml` · `csv` · `txt` · `lines` · `raw` (output-only) · `http` (input-only)

### CSV Options
| Option | Description |
|--------|-------------|
| `--csv-delimiter <char>` | Input separator (default: `,`; use `\t` for tabs) |
| `--csv-output-delimiter <char>` | Output separator (defaults to input delimiter) |
| `--csv-no-input-header` | First line is data, columns named `col0`, `col1`, ... |
| `--csv-no-output-header` | Don't write header row in output |
| `--csv-header "a,b,c"` | Explicit column names |

### HTTP Options
| Option | Description |
|--------|-------------|
| `--http-header "Name: Value"` | Custom HTTP header (repeatable) |
| `--timeout <seconds>` | Request timeout (default: 30) |
| `--no-follow` | Don't follow HTTP redirects |

### Message Verbosity
| Option | Description |
|--------|-------------|
| `--quiet` | Suppress all msg_* output except msg_error |
| `--msg-level=verbose` | Also show msg_verbose() output |
| `--msg-level=trace` | Also show msg_verbose() and msg_trace() output |

### Arguments & Environment
| Option | Description |
|--------|-------------|
| `--arg name=value` | Pass variable (access via `args["key"]`; repeatable) |
| `--env PATTERN` | Filter env vars by glob (`*`, `PREFIX_*`, `EXACT`, comma-separated) |

## Built-in Molds (Official Registry)

After running `fimod registry setup`, these molds are available with the `@` prefix:

### HTTP & APIs
| Mold | Description | Key args |
|------|-------------|----------|
| `@gh_latest` | GitHub latest release tag + asset URL resolution | `--arg repo=owner/repo`, `--arg asset=pattern` |
| `@download` | wget-like file download | `--arg out=filename` |

### DevOps & Config
| Mold | Description | Key args |
|------|-------------|----------|
| `@yaml_merge` | Patch YAML with dot-path assignments | `--arg set="spec.replicas=3,metadata.labels.env=prod"` |
| `@env_to_dotenv` | Convert config to `.env` format | — |
| `@poetry_migrate` | Migrate Poetry pyproject.toml to uv/Poetry 2 | `--arg target=uv\|poetry2` |
| `@skylos_to_gitlab` | Skylos dead code report → GitLab Code Quality JSON | — |

### Data Exploration
| Mold | Description | Key args |
|------|-------------|----------|
| `@jq_compat` | jq-like get/map/select operations | `--arg get=path`, `--arg map=field`, `--arg select=field=value` |
| `@json_schema_extract` | Extract simplified JSON schema | — |
| `@flatten_nested` | Flatten nested JSON to dot-path keys | — |
| `@deep_pluck` | Extract nested fields by dotpath | `--arg paths=user.name,user.email` |
| `@sort_json_keys` | Recursively sort JSON object keys | — |
| `@markdown_toc` | Extract markdown table of contents | — |
| `@log_parse` | Parse log lines via regex | `--arg regex=...`, `--arg fields=...` |

### Data Quality
| Mold | Description | Key args |
|------|-------------|----------|
| `@anonymize_pii` | Hash specified fields with SHA-256 | `--arg fields=email,phone` |
| `@validate_fields` | Validate required fields exist | `--arg required=field1,field2` |
| `@dedup_by` | Deduplicate by field | `--arg field=id` |
| `@pick_fields` | Keep only specified fields | `--arg fields=id,name,email` |
| `@rename_keys` | Rename keys via mapping | `--arg mapping=old:new,old2:new2` |
| `@split_tags` | Split tags field into list | `--arg field=tags`, `--arg sep=regex` |
| `@group_count` | Group by field and count | `--arg field=status` |

### CSV
| Mold | Description | Key args |
|------|-------------|----------|
| `@csv_to_json_records` | CSV → JSON array of objects | — |
| `@csv_stats` | Compute numeric column statistics (min, max, mean, count) | — |

## Built-in Helpers (Available automatically)

You do not need to `import` anything, these functions are globally available:

### Map / Dict Navigation (`dp_*`)
- **`dp_get(data, "a.b.c", default=None)`**: Safely get nested values without `KeyError`. Indexes work too: `"users.0.name"`, `"items.-1"`.
- **`dp_set(data, "a.b.c", value)`**: Return a deep copy of data with the mutated value. Missing intermediate keys are created automatically.

### Iteration & Collections (`it_*`)
Note: `it_group_by`, `it_sort_by`, and `it_unique_by` take a **string field name**, not a lambda!
- **`it_unique(list)`**: Deduplicate a primitive list.
- **`it_unique_by(list, "email")`**: Deduplicate a list of dicts based on the "email" field.
- **`it_sort_by(list, "created_at")`**: Sort a list of dicts (stable sort).
- **`it_group_by(list, "department")`**: Groups into a dict of lists: `{"engineering": [...], "sales": [...]}`
- **`it_keys(dict)` / `it_values(dict)`**: Get list of keys/values from dictionary.
- **`it_flatten(list)`**: Recursively flatten nested arrays `[[1, 2], [3]]` -> `[1, 2, 3]`.

### Regex (`re_*`)
Patterns use [fancy-regex](https://github.com/fancy-regex/fancy-regex) (PCRE2 flavour: lookahead, lookbehind, atomic groups). Two replacement syntaxes: `re_sub` uses Python `\1`/`\g<name>`; `re_sub_fancy` uses `$1`/`${name}`.
- **`re_search(r"...", text)`**: Returns `{"match": str, "start": int, "end": int, "groups": [...], "named": {...}}` or `None`.
- **`re_match(r"...", text)`**: Same as `re_search`, anchored to start of text.
- **`re_findall(r"...", text)`**: No groups -> `[str, ...]`. 1 group -> `[group_val, ...]`. N groups -> `[[g1, g2], ...]`.
- **`re_sub(r"...", r"\1", text [, count])`**: Python syntax (`\1`, `\g<name>`). Optional `count` (0=all).
- **`re_sub_fancy(r"...", "$1", text [, count])`**: fancy-regex syntax (`$1`, `${name}`). Optional `count`.
- **`re_split(r"...", text)`**: Captured groups are included in the result (Python behaviour).

### Hashing (`hs_*`)
- **`hs_sha256(text)` / `hs_md5(text)` / `hs_sha1(text)`**: Returns lowercase hex digest.

### Message Logging (`msg_*`)
Output diagnostic messages to stderr without affecting the data pipeline:
- **`msg_print(text)`**: Print to stderr (no prefix). Always visible unless `--quiet`.
- **`msg_info(text)`**: Print with `[INFO]` prefix. Always visible unless `--quiet`.
- **`msg_warn(text)`**: Print with `[WARN]` prefix. Always visible unless `--quiet`.
- **`msg_error(text)`**: Print with `[ERROR]` prefix. Always visible (even with `--quiet`).
- **`msg_verbose(text)`**: Print with `[VERBOSE]` prefix. Visible with `--msg-level=verbose` or `trace`.
- **`msg_trace(text)`**: Print with `[TRACE]` prefix. Visible only with `--msg-level=trace`.

### Validation Gates (`gk_*`)
Assert conditions and control pipeline failure. The mold continues executing after failure (lets you collect multiple errors):
- **`gk_fail(msg)`**: Emit `[ERROR] msg` to stderr, set exit code to 1.
- **`gk_assert(cond, msg)`**: If `cond` is falsy (Python truthiness), behave like `gk_fail(msg)`.
- **`gk_warn(cond, msg)`**: If `cond` is falsy, emit `[WARN] msg` to stderr (no exit).

### Environment Substitution
- **`env_subst(template, dict)`**: Replace `${VAR}` placeholders in template using dict values. Unknown vars left as-is.

### Flow Control
- **`set_exit(code)`**: Set the process exit code (useful with `--check` for custom validation).
- **`set_input_format("json"|"yaml"|...)`**: Re-parse the current step's output as the given format before the next step.
- **`cast_input_format("json", value)`**: Same as `set_input_format` but returns `value` — useful as a one-liner.
- **`set_output_format("json"|"yaml"|...|"raw")`**: Override the final output format from within the script.
- **`set_output_file(path)`**: Redirect output to a file (overrides `-o`).

### Transform Parameters
All mold scripts receive: `def transform(data, args, env, headers):`
- **`data`** — parsed input (dict, list, str, int, float, bool, None)
- **`args`** — dict of `--arg name=value` pairs (always `{}` if none passed)
- **`env`** — dict of filtered environment variables via `--env PATTERN` (`{}` without `--env`)
- **`headers`** — list of CSV column names, or `None` for non-CSV input

## Monty Python Subset Notes

The embedded Monty interpreter supports a Python subset:
- Core types, control flow, comprehensions, f-strings, string methods work
- **No file I/O, no network calls, no imports** — all I/O stays in Rust (security boundary)
- Dict merging: use `.update()` instead of `{**a, **b}` or `a | b` (not supported)
- `sorted()`, `min()`, `max()`, `len()`, `range()`, `enumerate()`, `zip()`, `type()`, `str()`, `int()`, `float()`, `bool()`, `list()`, `dict()` are available

## Security Model

Mold scripts are **pure functions** — they receive data and return a result. They cannot read/write files, access the network, or import external libraries. All I/O stays in Rust. You can safely run molds from remote URLs.

---

Whenever you need to inspect, query, or modify structured files in this repository, prefer using `fimod`.
