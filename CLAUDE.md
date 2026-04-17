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
- Dynamic shell completions use `clap_complete` `CompleteEnv` (activated via `COMPLETE=<shell>` env var). `fimod completions <shell>` prints activation instructions. Custom completers provide contextual completion for format names, `@mold` references, and registry source names.
- CLI uses `Option<Commands>` for subcommands: `None` = shape mode, `Some(Registry{..})` = registry management (list/add/show/remove/set-priority/build-catalog/setup), `Some(Mold{..})` = mold browsing/testing.
- Mold description is extracted from the module-level docstring (`"""..."""`) by `parse_mold_defaults()` into `MoldDefaults.docs`, used by `fimod mold list` (local scan) and `catalog.toml` (remote registries). `# fimod: description=` is no longer supported.
- `--output-format raw` short-circuits the entire transform pipeline (no mold allowed): fetches URL bytes directly or reads a file as binary and writes to `-o`. `set_output_format("raw")` from within a mold triggers the same binary pass-through but requires `--input-format http` to have populated `http_raw_bytes`.
- `DataFormat::Txt` serializes `Value::String` as a bare string (no JSON quotes); non-strings fall back to compact JSON. Use `--output-format txt` when piping a mold's string output to another command or to `-i`.
- Monty is a git dependency (not a stable crate) — its API may change.

## Testing

Integration tests: `tests/cli/<module>.rs` files, referenced by `tests/cli.rs`. Unit tests: embedded in `format.rs`. Mold fixture tests: `tests-molds/` (see `mold-tests` skill for fixture format details).

```bash
cargo test --test cli http          # Run CLI tests matching "http"
cargo test --lib format             # Run unit tests in format.rs matching "format"
cargo test --test molds_test        # All mold fixture tests
```

## Code Style

- Prefer dedicated function variants over boolean/mode parameters when behaviors diverge significantly (existing pattern: `re_sub` vs `re_sub_fancy`).

## Workflow

After implementing or modifying a feature, always:
1. Run `cargo clippy` and `cargo test` before considering the task complete.
2. Check if documentation needs updating (README.md, docs/built-ins.md, docs/cli-reference.md, docs/mold-scripting.md) and propose the changes.
3. When updating ROADMAP.md, move completed items to the appropriate documentation files (built-ins.md, cli-reference.md, etc.) rather than just marking them as done in the roadmap.

## Release

Use the `/release-workflow` skill — it orchestrates the full flow step by step and enforces the invariants below.

**Mandatory invariants:**

- Feature/fix work goes through a PR on a dedicated branch. Never commit work directly on `main`.
- `CHANGELOG.md` is updated ONLY in the `chore(release): X.Y.Z` commit — never in feature/fix commits.
- The `chore(release): X.Y.Z` commit contains ONLY `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`. It is made directly on `main` (no PR).
- Release commit subject is EXACTLY `chore(release): X.Y.Z` — never `fix:`, `feat:`, etc.
- Tag `vX.Y.Z` is created on `main` right after the `chore(release)` commit, then both are pushed.
