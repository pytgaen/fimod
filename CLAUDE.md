# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Fimod is a Rust CLI that transforms structured data files by executing Python "mold" scripts via [Monty](https://github.com/pydantic/monty) (Pydantic's embedded Python engine). No system Python installation required.

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo test                     # All tests (unit + integration)
cargo test --lib               # Unit tests only
cargo test --test cli           # Integration tests only
cargo test <test_name>          # Single test by name
cargo build --release           # Optimized release binary
```

Task runner (`task`) is also available — see `Taskfile.yml` for `task test`, `task build:release`, `task doc:serve`, etc.

## Architecture

All data flows through a single pipeline: **Read → Parse → Convert → Execute mold → Convert back → Serialize → Write**.

The intermediate representation between formats is always `serde_json::Value`. Monty operates on `MontyObject` (Python dicts).

```
src/
├── main.rs          — CLI (clap derive) + pipeline orchestration + subcommand routing
├── convert.rs       — serde_json::Value ↔ MontyObject bidirectional conversion
├── engine.rs        — Monty execution engine (iterative start/run loop for external function dispatch)
├── format.rs        — DataFormat enum, auto-detection by extension, parse/serialize for JSON/YAML/TOML/CSV/TXT
├── mold.rs          — MoldSource enum (File/Url/Inline), script loading + inline auto-wrapping
├── regex.rs         — External functions: re_search/match/findall/sub/split + _fancy variants (via fancy-regex)
├── dotpath.rs       — dp_get, dp_set (nested dotpath access)
├── iter_helpers.rs  — it_keys, it_values, it_flatten, it_group_by, it_sort_by, it_unique, it_unique_by
├── hash.rs          — hs_md5, hs_sha256, hs_sha1
├── template.rs      — tpl_render_str, tpl_render_from_mold (Jinja2 templating via minijinja)
├── msg.rs           — msg_print, msg_info, msg_warn, msg_error (stderr logging)
├── gatekeeper.rs    — gk_fail, gk_assert, gk_warn (validation gates with exit code control)
├── env_helpers.rs   — env_subst (${VAR} template substitution)
├── exit_control.rs  — set_exit() external function
├── format_control.rs — External functions: set_input_format, set_output_format, set_output_file, cast_input_format (format/output overrides from within a mold)
├── http.rs          — HTTP fetch (reqwest blocking), HttpResponse struct, content-type → format mapping, URL detection
├── pipeline.rs      — Pipeline orchestration: build_scripts, execute_chain, run_pipeline (MoldStep-based)
├── test_runner.rs   — `fimod mold test` runner: discovers test cases from a directory, executes mold against each, diffs output
└── registry.rs      — Registry system (~/.config/fimod/sources.toml + FIMOD_REGISTRY env var); resolves `@name` / `@source/name` mold references; Catalog/CatalogEntry for catalog.toml (remote registries); `registry setup` command for first-run onboarding
```

**Key design decisions:**
- All parsing/serialization stays in Rust (serde). Monty only manipulates Python dicts — this is a security boundary.
- Mold scripts must define a `transform(data, ...)` function. `args`, `env`, and `headers` are passed as keyword arguments, so molds only need to declare what they use (e.g. `def transform(data, args, **_):`). Inline expressions (`-e`) are auto-wrapped into this form.
- `--arg name=value` populates the `args` parameter — explicit access via `args["key"]`.
- `--env PATTERN` populates the `env` parameter with filtered environment variables (glob patterns: `*`, `PREFIX_*`, `EXACT`, comma-separated). Without `--env`, `env` is `{}`.
- CSV column names are passed as the `headers` parameter (list of strings, or `None` for non-CSV).
- External functions (regex) use Monty's iterative start()/state.run() loop, not the simple runner.run().
- `--debug` outputs to stderr with `[debug]` prefix; in debug mode Monty's print() also goes to stderr via custom `StderrPrint` (implements `PrintWriter`).
- `--in-place` rewrites the input file; output format auto-detection uses the input path.
- `--csv-output-delimiter` is separate from `--csv-delimiter` via `CsvOptions::effective_output_delimiter()`.
- Shell completions use `clap_complete`; `--completions <SHELL>` generates and exits early.
- CLI uses `Option<Commands>` for subcommands: `None` = shape mode, `Some(Registry{..})` = registry management (list/add/show/remove/set-default/build-catalog/setup), `Some(Mold{..})` = mold browsing/testing.
- Mold description is extracted from the module-level docstring (`"""..."""`) by `parse_mold_defaults()` into `MoldDefaults.docs`, used by `fimod mold list` (local scan) and `catalog.toml` (remote registries). `# fimod: description=` is no longer supported.
- `--output-format raw` short-circuits the entire transform pipeline (no mold allowed): fetches URL bytes directly or reads a file as binary and writes to `-o`. `set_output_format("raw")` from within a mold triggers the same binary pass-through but requires `--input-format http` to have populated `http_raw_bytes`.
- `DataFormat::Txt` serializes `Value::String` as a bare string (no JSON quotes); non-strings fall back to compact JSON. Use `--output-format txt` when piping a mold's string output to another command or to `-i`.
- Monty is a git dependency (not a stable crate) — its API may change.

## Testing

Integration tests are split across `tests/cli/<module>.rs` files, referenced by `tests/cli.rs` (the entry point). They use `assert_cmd` + `assert_fs` + `predicates`: create temp files, invoke the `fimod` binary, assert on stdout/stderr/exit codes. Unit tests for format handling are embedded in `format.rs`. Mold registry tests use a custom `HOME` env var to isolate from the user's real config.

```bash
cargo test --test cli http          # Run CLI tests matching "http"
cargo test --lib format             # Run unit tests in format.rs matching "format"
```

### Mold fixture tests (`tests-molds/`)

`tests/molds_test.rs` auto-discovers and runs all fixture cases under `tests-molds/`. Each subdirectory must match a mold in `molds/{name}/`. Cases are built from `{case}.input.*` + `{case}.expected.*` file pairs. An optional `{case}.run-test.toml` enriches a case with args, env vars, exit code, or format overrides:

```toml
# tests-molds/my_mold/basic.run-test.toml
[args]
field = "email"

# [env_vars]
# MY_VAR = "value"

# output_format = "json-compact"
# exit_code = 1
# skip = true
```

Without a `.run-test.toml`, the case runs with no args and expects exit code 0.

## Design Philosophy

When proposing names, labels, CLI flags, or output formats, always reason from the end-user perspective first. Before proposing a design, show a concrete example of what the user will actually see — a terminal snippet, a CLI invocation with realistic output, or a screenshot-equivalent text mock-up. Ask yourself: "If I'm a user running this command for the first time, does this make sense? Does it look clean?" If the answer is no, iterate before proposing.

Prefer simple, single-concept designs (one flag, one variant function) over complex multi-parameter approaches. When in doubt, show two concrete alternatives side by side and let the user pick.

## Codebase Exploration

To understand the codebase structure without reading full files, use the LSP tool with rust-analyzer:
- `LSP documentSymbol` on a file to list all functions, structs, enums
- `LSP workspaceSymbol` to search for a symbol across the project
- `LSP hover` for type info and docs
- `LSP findReferences` / `LSP incomingCalls` / `LSP outgoingCalls` for usage and call graphs

The `/explore` skill wraps this into a single command.

## Code Style

- Prefer dedicated function variants over boolean/mode parameters when behaviors diverge significantly (existing pattern: `re_sub` vs `re_sub_fancy`).

## Workflow

After implementing or modifying a feature, always:
1. Run `cargo clippy` and `cargo test` before considering the task complete.
2. Check if documentation needs updating (README.md, docs/built-ins.md, docs/cli-reference.md, docs/mold-scripting.md) and propose the changes.
3. When updating ROADMAP.md, move completed items to the appropriate documentation files (built-ins.md, cli-reference.md, etc.) rather than just marking them as done in the roadmap.

## Language

DESIGN_NOTES.md, ROADMAP.md and commit messages are in English. Code, docs, and CLAUDE.md are in English.

## RTK — CLI proxy for verbose output

`rtk` is installed and compresses verbose command output to save tokens. Use it proactively.

**Always prefix with `rtk`:**
- Git: `rtk git status`, `rtk git diff`, `rtk git log`, `rtk git show`
- Tests: `rtk cargo test`, `rtk cargo build`
- Listings/search: `rtk find`, `rtk grep -R` (on large trees)

**Never prefix with `rtk`:**
- Interactive tools: `vim`, `less`, `tmux`, REPLs
- Commands where exact output matters for debugging

**If output is too condensed** to diagnose an issue, re-run without `rtk` and say so.
Use `rtk gain` to check token savings per command.
