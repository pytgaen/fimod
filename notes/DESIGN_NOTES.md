# Fimod — Design Notes

Design notes: choices made, their reasons, and known constraints.

## Design Decisions

### Single Pipeline

The entire flow goes through: **Read → Parse → Convert → Execute mold → Convert back → Serialize → Write**.

The intermediate representation between formats is always `serde_json::Value`. Monty operates on `MontyObject` (Python dicts).

The single source of truth is `run_pipeline_core()` in `pipeline.rs`. `process_single_input()` (CLI) and `run_pipeline()` (library API) both delegate to it. The sequence:

1. **Read** — file, stdin, or HTTP URL
2. **Format detection** — CLI arg > Content-Type (HTTP) > extension > fallback (JSON)
3. **HTTP short-circuit** — if `--input-format http`, build an HTTP response dict and skip parsing
4. **Parse** — to `Value`, handling special cases (slurp, NDJSON, CSV)
5. **Convert** — `Value` → `MontyObject`
6. **Execute chain** — run all molds sequentially (see *Mold chaining*)
7. **Format override** — apply `set_input_format()` / `set_output_format()` from mold
8. **Serialize** — `Value` → String (or raw binary pass-through)
9. **Write** — file, directory, stdout, or `set_output_file()` target

Exception: the exact single-step identity expression `-e 'data'` is a native
format-conversion path. It keeps the explicit identity syntax, but runs
**Read → Parse → Serialize → Write** in Rust without converting to
`MontyObject` and without starting Monty. JSON-array → NDJSON and NDJSON →
JSON/JSON-compact conversions stream item by item when their input/output paths
are distinct; JSON-array → CSV uses its existing streaming writer and column
scan window. Any other expression, mold, or multi-step chain uses the normal
mold pipeline.

### Security: parsing in Rust, logic in Python

All parsing/serialization (serde) remains entirely in Rust. Monty only manipulates Python dicts. This is a deliberate security boundary: user scripts never have access to the filesystem or the network.

Concretely, `engine.rs` enforces this at the VM boundary: every `RunProgress::OsCall` yielded by Monty is routed through `dispatch_os_call()` and checked against the resolved `SandboxPolicy`. Clock access (`datetime.now`, `date.today`) is opt-in via `allow_clock`; `os.getenv` only returns values allowed by `allow_env`; `os.environ` returns an empty dict when denied; filesystem calls still return `None` until mount-based access exists. Resource limits are applied by default (`10m` / `2GB`) unless the policy explicitly changes them.

### `transform(data, **_)` with kwargs

Molds define `transform(data, **_)`. fimod passes `args`, `env`, `headers`, and `pipeline` as **keyword arguments**, so reusable molds should keep `**_` and only declare the named parameters they actually use — `def transform(data, **_)`, `def transform(data, args, **_)`, `def transform(data, pipeline, **_)`, etc. Inline `-e` expressions are auto-wrapped into a `transform(..., **_)` function.

- `args` dict ← `--arg name=value` (explicit `args["threshold"]`, no magic globals)
- `env` dict ← `--env PATTERN` filtered environment (empty `{}` without `--env`)
- `headers` list ← CSV column names (`None` for non-CSV)
- `pipeline` object ← current/future pipeline state for dynamic molds

### Mold chaining

Multiple `-m` and `-e` arguments execute sequentially; the output of each step becomes the input of the next (`execute_chain` in `pipeline.rs`). Between steps, the hot path threads `MontyObject` directly. If `set_input_format()` was called, the result is converted, serialized, and re-parsed with the new format. The `"raw"` output format is restricted to the final step only.

The `-e 'data'` identity optimization only applies when it is the whole chain.
Inside a multi-step chain it remains a normal inline expression so step counts
and pipeline metadata stay stable.

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
| `dp_` | `dotpath.rs` | `dp_get`, `dp_set`, `dp_has`, `dp_delete` (nested dotpath access, negative indices, auto-create intermediates) |
| `it_` | `iter_helpers.rs` | `it_keys`, `it_values`, `it_flatten`, `it_group_by`, `it_sort_by`, `it_unique`, `it_unique_by`, `it_count_by`, `it_min_by`, `it_max_by` |
| `hs_` | `hash.rs` | `hs_md5`, `hs_sha1`, `hs_sha256` (hex lowercase) |
| `gk_` | `gatekeeper.rs` | `gk_fail`, `gk_assert`, `gk_warn` (validation gates with exit code control) |
| `msg_` | `msg.rs` | `msg_print`, `msg_info`, `msg_warn`, `msg_error`, `msg_verbose`, `msg_trace` |
| `tpl_` | `template.rs` | `tpl_render_str`, `tpl_render_from_mold` (Jinja2 templating via minijinja) |
| — | `env_helpers.rs` | `env_subst` (`${VAR}` template substitution) |
| — | `exit_control.rs` | `set_exit(code)` |
| — | `format_control.rs` | `set_input_format`, `set_output_format`, `set_output_file`, `cast_input_format` |

### `fancy-regex` rather than Rust `regex`

Fimod's `re_*` built-ins predate Monty's `import re` support and remain useful because they expose structured results, explicit replacement modes, and ReDoS protection via `FIMOD_REGEX_BACKTRACK_LIMIT` (default: 100k). Rust's standard `regex` crate is not PCRE2-compatible (no backreferences, lookahead, lookbehind), so the built-ins use `fancy-regex`. Python replacement syntax (`\1`, `\g<name>`) is auto-converted for `re_sub`; `re_sub_fancy` exposes `$1` / `${name}` directly.

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

`execute_mold` returns `MoldExecResult`: the step result as `MontyObject`, optional exit code, output format/file overrides, plus pending dynamic pipeline steps and mutations.

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

Separate `--csv-delimiter` (input) and `--csv-output-delimiter` (output,
defaults to input). `--csv-no-input-header`, `--csv-no-output-header`,
`--csv-header col1,col2` (input columns and object-output projection),
`--csv-scan N` (object-output column scan, default `1`, `0` = all rows).

`serde_json/preserve_order` in `Cargo.toml` so that CSV column order is preserved through the pipeline.

### HTTP input and binary pass-through

`--input-format http` fetches a URL via reqwest (blocking) and builds an `HttpResponse` struct with status, headers, body (or `null` for binary), `body_bytes`, `body_size`, `content_type`. Content-Type is mapped to a `DataFormat` for auto-detection.

`--output-format raw` writes bytes directly (no serialization). With `-O` (`--url-filename`), output filenames are derived from URLs.

### Process-wide in-memory caches

Compiled regexes, inline templates and HTTP clients use fixed-capacity LRU
caches, including during a single pipeline invocation: 256 regexes, 64
templates and 16 HTTP clients. Eviction only lowers the hit rate; a missing
entry is compiled or created again, so results do not depend on cache state.

### MoldDefaults: metadata from mold scripts

`parse_mold_defaults()` in `mold.rs` extracts `# fimod:` directives from the mold preamble:
- `input-format`, `output-format`, `csv-*`, `no-follow`, `arg`, `env`
- Module-level docstring (`"""..."""`) is extracted as the `docs` field (used by `fimod mold list` and `catalog.toml`)
- First mold in a chain → input/CSV options; last mold → output/compact/raw options.
- `key!=value` marks a directive as **forced** — the CLI cannot override it. Forced directive names are stored in `MoldDefaults.forced: HashSet<String>`. Verbose warning (`--msg-level verbose`) is emitted when a forced directive is active.

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
- `fimod setup registry defaults` handles first-run onboarding; `fimod registry setup` is only a deprecated compatibility alias.
- `fimod setup sandbox defaults/show/get/set` manages sandbox policy files. `defaults` writes named presets, `show` renders normalized TOML, `get` prints one value, and `set` updates selected fields in the canonical file or `--sandbox-file <PATH>`.
- `fimod setup all defaults --if-needed` is the post-install onboarding contract used by install scripts. The binary, not shell/PowerShell glue, owns `FIMOD_SETUP_ALL`, `FIMOD_SETUP_REGISTRY`, `FIMOD_SETUP_SANDBOX`, interactive prompts, and idempotent upgrade skips.
- Subcommands: `list`, `add`, `show`, `remove`, `set-priority`, `build-catalog`, `cache`.

### Mold test runner

`fimod mold test <mold> <tests_dir>` discovers test cases from `*.input.*` + `*.expected.*` file pairs. An optional `*.run-test.toml` enriches a case with args, env vars, format overrides, exit code, or `skip = true`.

### Monty REPL

`fimod monty repl` provides an interactive Python REPL using Monty's
`MontyRepl::new()` with continuation mode detection. It resolves the same
sandbox policy as `fimod s`, including `--sandbox-file <path>` and
`--sandbox-file=""`. Mold-only external helper families such as `re_*` and
`dp_*` are not imported into the REPL.

### Debug on stderr

`--debug` prints to stderr with a `[debug]` prefix. In debug mode, Python's `print()` also goes to stderr via `StderrPrint` (implements `PrintWriter`). This never corrupts stdout.

### Shell completions

Dynamic shell completions via `clap_complete` `CompleteEnv`. When the `COMPLETE=<shell>` env var is set, the binary generates a shell-specific completion script and exits. `fimod setup completions --shell <shell>` prints an activation script suitable for `eval`. Custom `ArgValueCompleter`s provide dynamic completion for `--input/output-format` (format names), `-m @<TAB>` (registry mold names), and registry source name arguments.

### Optional subcommand

CLI uses `Option<Commands>`: `Some(Shape(..))` = pipeline, `Some(Registry{..})` = registry management, `Some(Mold{..})` = mold browsing/testing, `Some(Monty{..})` = REPL, `Some(Setup{..})` = setup helpers. `None` prints help and exits with code 2.

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

All build tools are managed by mise: `rust`, `zig`, `upx`, `uv`. `mise.toml` pins Rust to `1.95` because Monty v0.0.18 requires that compiler baseline. `rust-toolchain.toml` pins the cross-compilation targets (read by rustup and mise). Windows packaging uses `uv run python3 -c "import zipfile; ..."` to avoid any system dependency.

## Watchpoints

- **Monty API pinned to tag**: Monty is a git dependency pinned to `v0.0.18` (tag in `Cargo.toml`; `MONTY_VERSION` is injected at build time via `env!("MONTY_VERSION")`). The `MontyRun::new` API and error types can change between releases. The `monty-upgrade` skill maps consumed APIs and flags breaking changes for each bump.
- **`num-bigint`** in `convert.rs`: `i64::try_from(BigInt)` conversion is used for large integers.
