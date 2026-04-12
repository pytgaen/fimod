# Changelog

All notable changes to fimod are documented here.

## [0.3.1] — 2026-04-12

### Bug Fixes

- **iter:** `it_group_by` preserves insertion order (switched to `IndexMap`) — grouped output follows data order instead of alphabetical.
- **mold:** `# fimod:` directives support quoted arg descriptions (`"..."` / `'...'`) so commas don't split entries.
- **cli:** `--mold=` / `--expression=` long-form flags now recognized by the pre-parse pass.
- **engine:** intercepted `OsCall` emits a `[debug]` message instead of failing silently.

### Features

- **mold:** `fimod mold show --output-format json` for machine-readable output.

### Documentation

- Expanded `formats.md` (lines vs txt vs NDJSON, shell-friendly recipes).
- Updated `cli-reference.md` and `built-ins.md`.

### Housekeeping

- Added `indexmap` dependency.
- Removed dead `parse_data` / `parse_file` from `pipeline.rs`.

## [0.3.0] — 2026-04-10

### Highlights

- 🔀 Migrate production molds to dedicated repo [fimod-powered](https://pytgaen.github.io/fimod-powered/) (`gh_latest`, `download`, `poetry_migrate`, `skylos_to_gitlab`). Use `fimod registry setup` to migrate
- 👀 **fimod-powered** - New molds showcasing Jinja2 templating (MiniJinja engine): `html_report`, `dockerfile`
- **📌 Priority-based registry resolution** - registries searched in priority order (P0→P99). New `set-priority` command replaces `set-default`.
- **🐚 Dynamic shell completions** - context-aware Tab completion for subcommands, flags, format names, `@mold` references, and registry source names.
- ⬆️ Update to **🐍 Monty v0.0.11**/cle

### Features

- **completions:** dynamic shell completions via `clap_complete` `CompleteEnv` — Tab-completes format names, `@mold` references, registry source names. New `fimod completions <shell>` subcommand.
- **registry:** `set-priority <name> <rank>` command for priority-based resolution. Bare `@mold` references resolved in priority order across all registries. Swap semantics by default; `--cascade` to shift others down.
- **registry:** `build-catalog` now takes a directory path as positional argument. `--registry <name>` resolves from a registered source.
- **registry:** duplicate URL/path detection in `registry add`.
- **registry:** `setup` migrates legacy "official" registry to "examples" (P99) and adds fimod-powered (P10).
- **registry:** catalog TTL cache (60s) — Tab completion and repeated calls skip HTTP when cache is fresh.
- **registry:** companion files support — remote molds with templates/data files are downloaded alongside the main script.
- **pipeline:** `-m` and `-e` can now be mixed and interleaved freely in CLI order.
- **pipeline:** `--input-format http` exposes `data["url"]`, `data["body_size"]`, `data["content_type"]`.
- **format:** CSV output supports array-of-arrays (`[[v1, v2], ...]`) with `--csv-header-names`.
- **install:** SHA-256 checksum verification for downloaded binaries.
- **monty:** upgrade from v0.0.9 to v0.0.11 (json perf, mount fixes, filesystem mounting, multi-module imports).

### Bug Fixes

- **registry:** fix resolution fallthrough when catalog exists but mold not found.
- **registry:** fix fimod-powered registry URL in setup (missing `/tree/main/molds`).
- **test-runner:** `fimod mold test` now resolves mold base directory for `tpl_render_from_mold()`.

### Breaking Changes

- **completions:** `--completions <shell>` flag removed. Use `COMPLETE=<shell> fimod` or `fimod completions <shell>`.
- **registry:** `fimod registry build-catalog <name>` is now `fimod registry build-catalog <path>`. Use `--registry <name>`.
- **registry:** `fimod registry set-default` removed — use `fimod registry set-priority <name> 0`.
- **license:** LGPL-3.0-only → Apache-2.0.
- **deps:** `serde_yaml` replaced by `serde-saphyr` (pure-Rust YAML).

## [0.2.0] — 2026-04-02

### Highlights

- **⬆️ Monty v0.0.9** — `import datetime`, `import json`, named keyword args (`key=`), nested subscript assignment. Date/DateTime/TimeDelta are auto-serialized as ISO 8601 strings in the output.
- **🧩 Jinja2 templating** — new `tpl_render_str` and `tpl_render_from_mold` built-ins for data→text generation via MiniJinja (inline strings or `.j2` files).
- **⚡ Mold cache** — registry molds are cached locally with ETag and content hash validation. No re-download on repeat runs.

### Features

- **monty:** upgrade to Monty v0.0.9 — `datetime`, `json` modules, named keyword args, nested subscript assignment
- **convert:** serialize Date, DateTime, TimeDelta, Timezone as ISO 8601 strings
- **template:** add Jinja2 templating engine (tpl_render_str, tpl_render_from_mold) — data→text generation via MiniJinja, inline strings or .j2 files, path traversal security, badge_md and git_changelog demo molds
- **cache:** add registry mold cache with ETag and content hash validation
- **engine:** pass `args`, `env`, `headers` as keyword arguments — molds only need to declare what they use (`def transform(data, args, **_):`)

### Changed

- **molds:** update all bundled mold signatures to use `**_` kwargs pattern
- **docs:** update monty-engine.md and mold-scripting.md for v0.0.9 capabilities
- **build:** extract MONTY_VERSION from Cargo.toml via build.rs (no more manual sync)
- **install:** migrate 'official' registry to 'examples' on upgrade
- **install:** skip version fetch when `FIMOD_SKIP_DOWNLOAD=1`

### Bug Fixes

- **core:** extract pipeline logic into lib.rs and pipeline.rs

## [0.1.2] — 2026-03-25

### Fixed

- **Registry: FIMOD_REGISTRY visibility** — `fimod registry list` and `fimod mold list` now display entries defined via the `FIMOD_REGISTRY` environment variable

### Changed

- **Installer: piped input support** — `install.sh` reads the registry setup prompt from `/dev/tty` when piped via `curl | sh`
- **Installer: reduced prompts** — Simplified confirmation questions in `install.sh` and `install.ps1`

## [0.1.1] — 2026-03-21

### Fixed

- **Windows: config path crash** — `registry.rs` now falls back to `USERPROFILE` when `HOME` is not set, preventing a panic on first run under Windows ([#3467])
- **Windows: cache path crash** — `mold.rs` applies the same `USERPROFILE` fallback for the mold cache directory ([#3468])

### Changed

- **Installers prompt before registry setup** — `install.sh` and `install.ps1` now ask for confirmation before running `fimod registry setup`, instead of executing it automatically
- **Quick Start guide: Windows installation** — Added two Windows installation options (PowerShell script and ubi-based) with platform-specific tabs and PATH configuration notes
- **README: Windows PATH instructions** — Explicit PowerShell commands to configure PATH when the installer cannot modify it automatically

## [0.1.0] — 2026-03-21

Initial release — fimod, a Rust CLI that transforms structured data files by
executing Python mold scripts via Monty (Pydantic's embedded Python engine).
No system Python installation required.

- Multi-format I/O: JSON, YAML, TOML, CSV/TSV, NDJSON, TXT, Lines
- Inline expressions (`-e`) and mold file (`-m`) execution
- Single pipeline: Read → Parse → Convert → Execute → Serialize → Write
- Intermediate representation: `serde_json::Value` / `MontyObject`

- `dp_get` / `dp_set` — safe nested dotpath access
- `re_search`, `re_match`, `re_findall`, `re_sub`, `re_split` — regex (+ fancy-regex variants)
- `it_keys`, `it_values`, `it_flatten`, `it_group_by`, `it_sort_by`, `it_unique`, `it_unique_by`
- `hs_md5`, `hs_sha256`, `hs_sha1` — hashing
- `gk_fail`, `gk_assert`, `gk_warn` — validation gates with exit code control
- `msg_print`, `msg_info`, `msg_warn`, `msg_error` — stderr logging
- `env_subst` — `${VAR}` template substitution
- `set_exit`, `set_format`, `set_input_format`, `set_output_file` — pipeline control

- HTTP input with raw response envelope (`--input-format http`, `--no-follow`, `--http-header`)
- Binary pass-through via `set_format("raw")` + `set_output_file()`
- Pipeline chaining (multiple `-e` / `-m`), slurp mode (`--slurp`), batch processing
- Mold registry: `~/.config/fimod/sources.toml` + `FIMOD_REGISTRY`, remote catalogs (`@name`, `@source/name`)
- `--check` mode for validation pipelines (exit 0/1 on truthy/falsy result)
- `--no-input` mode for data generation
- `--in-place` rewrite, `--compact` output, `--debug` mode
- Shell completions: bash, zsh, fish, powershell (`--completions <SHELL>`)
- CSV options: delimiter, output-delimiter, header control
- `--arg name=value` and `--env PATTERN` for parameterized molds
- Guides, reference, examples (JSON, YAML, CSV, HTTP) and cookbook — MkDocs Material site
