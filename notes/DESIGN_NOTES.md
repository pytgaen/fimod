# Fimod — Design Notes

Design notes: choices made, their reasons, and known constraints.

## Design Decisions

### Single Pipeline

The entire flow goes through: **Read → Parse → Convert → Execute mold → Convert back → Serialize → Write**.

The intermediate representation between formats is always `serde_json::Value`. Monty operates on `MontyObject` (Python dicts).

In practice, the pipeline in `process_single_input()` is:

1. **Read** — file, stdin, or HTTP URL
2. **Format detection** — CLI arg > Content-Type (HTTP) > extension > fallback (JSON)
3. **HTTP short-circuit** — if `--input-format http`, build an HTTP response dict and skip parsing
4. **Parse** — to `Value`, handling special cases (slurp, NDJSON, CSV)
5. **Convert** — `Value` → `MontyObject`
6. **Execute chain** — run all molds sequentially (see *Mold chaining*)
7. **Format override** — apply `set_input_format()` / `set_output_format()` from mold
8. **Serialize** — `Value` → String (or raw binary pass-through)
9. **Write** — file, directory, stdout, or `set_output_file()` target

### Security: parsing in Rust, logic in Python

All parsing/serialization (serde) remains entirely in Rust. Monty only manipulates Python dicts. This is a deliberate security boundary: user scripts never have access to the filesystem or the network.

### Global `args` dict (not individual variables)

`--arg name=value` injects a global `args` dict, not individual Python variables:
- Explicit: `args["threshold"]` rather than a magic `threshold`
- Does not change the `transform(data)` signature
- Works with `-e` and `-m` without adaptation

### Mold chaining

Multiple `-m` and `-e` arguments execute sequentially; the output of each step becomes the input of the next (`execute_chain` in `main.rs`). Between steps, if `set_input_format()` was called, the result is re-serialized then re-parsed with the new format. The `"raw"` output format is restricted to the final step only.

### Batch mode (multiple inputs)

Multiple `-i` inputs are processed sequentially, each running the full pipeline. Batch mode requires either `-o <directory>` or `--in-place`. Per-file aliases are supported with `path:alias` syntax.

### Multi-file slurp

`-s` (slurp) with multiple `-i` combines all files into a single data structure before executing the mold:
- **Named mode** (`file1:key1 file2:key2`) → `{"key1": …, "key2": …}`
- **Array mode** (no aliases) → `[file1_data, file2_data, …]`
- Mixing alias and non-alias entries is an error.

### Input list mode

`--input-list FILE` (or `-I -` for stdin) reads input paths one per line. Lines starting with `#` are ignored, whitespace is trimmed.

### Check mode

`--check` evaluates the mold and exits 0 if the result is truthy, 1 if falsy. No output is written to stdout. Falsy values: `null`, `false`, `0`, `""`, `[]`, `{}`.

### External functions via iterative loop

External functions (regex, etc.) use Monty's iterative loop `start()` / `state.run()`, not the simple `runner.run()`. When Python calls an external function, Monty pauses execution, Rust dispatches the call via `dispatch_external()`, then resumes.

### External function catalog

Convention: 2-letter prefix to avoid collisions with Python builtins and make the origin explicit.

| Prefix | Module | Functions |
|--------|--------|-----------|
| `re_` | `regex.rs` | `re_search`, `re_match`, `re_findall`, `re_sub`, `re_split` + `_fancy` variants |
| `dp_` | `dotpath.rs` | `dp_get`, `dp_set` (nested dotpath access, negative indices, auto-create intermediates) |
| `it_` | `iter_helpers.rs` | `it_keys`, `it_values`, `it_flatten`, `it_group_by`, `it_sort_by`, `it_unique`, `it_unique_by` |
| `hs_` | `hash.rs` | `hs_md5`, `hs_sha1`, `hs_sha256` (hex lowercase) |
| `gk_` | `gatekeeper.rs` | `gk_fail`, `gk_assert`, `gk_warn` (validation gates with exit code control) |
| `msg_` | `msg.rs` | `msg_print`, `msg_info`, `msg_warn`, `msg_error`, `msg_verbose`, `msg_trace` |
| — | `env_helpers.rs` | `env_subst` (`${VAR}` template substitution) |
| — | `exit_control.rs` | `set_exit(code)` |
| — | `format_control.rs` | `set_input_format`, `set_output_format`, `set_output_file`, `cast_input_format` |

### `fancy-regex` rather than `regex`

Monty does not support `import re`. Rust's `regex` crate is not PCRE2-compatible (no backreferences, lookahead, lookbehind). `fancy-regex` offers the best compromise: pure Rust, no C dependency, covers the most used PCRE2 features. ReDoS protection via `FIMOD_REGEX_BACKTRACK_LIMIT` (default: 100k). Python replacement syntax (`\1`, `\g<name>`) is auto-converted.

### Message levels

`msg_error` is always shown. Other `msg_*` functions depend on `--msg-level` (verbose, trace) and `--quiet`. This is separate from `--debug` (which controls Monty internals).

### Gatekeeper: validation gates

`gk_fail(msg)` exits immediately, `gk_assert(cond, msg)` exits if falsy, `gk_warn(cond, msg)` logs without exiting. All output to stderr.

### Format control from within a mold

Four external functions let a mold override pipeline behavior at runtime:
- `set_input_format(name)` — re-parse result between chain steps
- `set_output_format(name)` — override output format (including `"raw"` for binary pass-through)
- `set_output_file(path)` — dynamically redirect output (override `-o`)
- `cast_input_format(name, value)` — single-expression combo of set + re-parse

`execute_mold` returns a 4-tuple: `(Value, Option<i32>, Option<String>, Option<String>)` — result, exit code, format override, output file override.

### Environment variable filtering (`--env`)

`--env PATTERN` populates the `env` parameter with filtered environment variables:
- `*` → all variables
- `PREFIX*` → prefix match
- `EXACT` → exact match
- Comma-separated: `HOME,PATH` or `GITHUB_*,CI`
- Without `--env`, `env` is `{}`.

### Data formats

Supported formats: JSON, JSON-compact, NDJSON, YAML, TOML, CSV, TXT, Lines, Raw, HTTP.

- **TXT** serializes `Value::String` as a bare string (no JSON quotes); non-strings fall back to compact JSON.
- **NDJSON** with `-s` (slurp) parses each line as a separate value into an array.
- **Raw** (`--output-format raw`) short-circuits the entire pipeline (no mold allowed for direct raw, or requires `--input-format http` to have populated `http_raw_bytes` when called from a mold via `set_output_format("raw")`).
- **HTTP** (`--input-format http`) fetches a URL and builds `{status, headers, body, body_size, content_type}`. Binary content sets `body = null`.

### CSV-specific options

Separate `--csv-delimiter` (input) and `--csv-output-delimiter` (output, defaults to input). `--csv-no-input-header`, `--csv-no-output-header`, `--csv-header col1,col2` (implies no-input-header).

`serde_json/preserve_order` in `Cargo.toml` so that CSV column order is preserved through the pipeline.

### HTTP input and binary pass-through

`--input-format http` fetches a URL via reqwest (blocking) and builds an `HttpResponse` struct with status, headers, body (or `null` for binary), `body_bytes`, `body_size`, `content_type`. Content-Type is mapped to a `DataFormat` for auto-detection.

`--output-format raw` writes bytes directly (no serialization). With `-O` (`--url-filename`), output filenames are derived from URLs.

### MoldDefaults: metadata from mold scripts

`parse_mold_defaults()` in `mold.rs` extracts `# fimod:` directives from the mold preamble:
- `input-format`, `output-format`, `csv-*`, `no-follow`, `arg`, `env`
- Module-level docstring (`"""..."""`) is extracted as the `docs` field (used by `fimod mold list` and `catalog.toml`)
- First mold in a chain → input/CSV options; last mold → output/compact/raw options.

### Mold resolution

`MoldSource` enum: `File`, `Url`, `Inline`.
- Directory molds: try `<dirname>/<dirname>.py`, fall back to `__main__.py`.
- URL molds: cached under `FIMOD_CACHE_DIR`, TTL controlled by `FIMOD_CACHE_TTL` (minutes; 0 = infinite, < 0 = disabled, default 360 = 6h). Cache key = SHA-256(url) + `.py`.
- Registry resolution: `@name` and `@registry/name` look up molds via the registry system.

### Registry system

`~/.config/fimod/sources.toml` stores named registries (local directories or remote GitHub/GitLab/HTTP).
- `@name` resolves via the default registry; `@source/name` resolves via a specific source.
- Auto-token detection for GitHub (`GITHUB_TOKEN`) and GitLab (`GITLAB_TOKEN`).
- Remote registries publish a `catalog.toml` for discovery.
- `fimod registry setup` handles first-run onboarding.
- Subcommands: `list`, `add`, `show`, `remove`, `set-default`, `build-catalog`, `setup`.

### Mold test runner

`fimod mold test <mold> <tests_dir>` discovers test cases from `*.input.*` + `*.expected.*` file pairs. An optional `*.run-test.toml` enriches a case with args, env vars, format overrides, exit code, or `skip = true`.

### Monty REPL

`fimod monty repl` provides an interactive Python REPL using Monty's `MontyRepl::new()` with continuation mode detection. All external functions are available.

### Debug on stderr

`--debug` prints to stderr with a `[debug]` prefix. In debug mode, Python's `print()` also goes to stderr via `StderrPrint` (implements `PrintWriter`). This never corrupts stdout.

### Shell completions

`--completions <SHELL>` (bash, zsh, fish, etc.) generates shell completion scripts via `clap_complete` and exits immediately.

### Optional subcommand

CLI uses `Option<Commands>`: `None` = shape mode (default pipeline), `Some(Registry{..})` = registry management, `Some(Mold{..})` = mold browsing/testing, `Some(Monty{..})` = REPL.

## CI / Build

### Gitea Actions — GHES constraints

`@v4` actions are not supported on Gitea (GHES):
- `actions/upload-artifact@v3` ✅ — `@v4` → `GHESNotSupportedError`
- `actions/download-artifact@v3` ✅
- `actions/cache@v3` ✅

Marketplace actions that run in a Docker container (e.g., `orhun/git-cliff-action`) fail because the container lacks network access for `apt-get`. Always download binaries manually via `curl` in a normal step.

To extract a binary from a tarball, prefer `find + xargs cp` over `tar --strip-components` because the internal structure of archives varies across releases:
```bash
curl -sSfL "URL" | tar xz -C /tmp/
find /tmp -name 'binary' -type f | head -1 | xargs -I{} cp {} /usr/local/bin/binary
```

pip cache (`~/.cache/pip`): always run `pip install` even on a cache hit — the pip cache speeds up downloading but does not replace installation.

### Taskfile — zig via mise

`cargo-zigbuild` looks for `zig` in the PATH, but `task` does not propagate PATH modifications via `env:`. Solution: use the environment variable `CARGO_ZIGBUILD_ZIG_PATH` which points directly to the binary:
```yaml
env:
  CARGO_ZIGBUILD_ZIG_PATH:
    sh: mise which zig 2>/dev/null || which zig
```

### Local tooling (mise.toml)

All build tools are managed by mise: `rust`, `zig`, `upx`, `uv`. `rust-toolchain.toml` pins the cross-compilation targets (read by rustup and mise). Windows packaging uses `uv run python3 -c "import zipfile; ..."` to avoid any system dependency.

## Watchpoints

- **Monty API pinned to tag**: Monty is a git dependency pinned to `v0.0.8` (both in `Cargo.toml` tag and `MONTY_VERSION` const). The `MontyRun::new` API and error types can change between releases.
- **`num-bigint`** in `convert.rs`: `i64::try_from(BigInt)` conversion is used for large integers.
